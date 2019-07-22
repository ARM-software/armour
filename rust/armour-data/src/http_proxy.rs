//! HTTP proxy with Armour policies

use super::policy::{self, ToArmourExpression};
use actix_web::client::{self, Client, ClientRequest, ClientResponse, SendRequestError};
use actix_web::{
    http::header::{HeaderName, HeaderValue},
    middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use futures::stream::Stream;
use futures::{future, Future};

pub fn start_proxy(
    policy: actix::Addr<policy::DataPolicy>,
    proxy_port: u16,
) -> std::io::Result<actix_web::dev::Server> {
    let socket_address = format!("0.0.0.0:{}", proxy_port);
    let server = HttpServer::new(move || {
        App::new()
            .data(policy.clone())
            .data(Client::new())
            .data(proxy_port)
            .wrap(middleware::Logger::default())
            .default_service(web::route().to_async(proxy))
    })
    .bind(&socket_address)?
    .start();
    log::info!("starting proxy server: http://{}", socket_address);
    Ok(server)
}

/// Main HttpRequest proxy
///
/// Checks request against Armour policy and, if accepted, forwards it using [ForwardUrl](trait.ForwardUrl.html).
/// The server response is then checked before it is forwarded back to the original client.
pub fn proxy(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<actix::Addr<policy::DataPolicy>>,
    client: web::Data<Client>,
    proxy_port: web::Data<u16>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    policy
        .send(policy::Evaluate::Require(
            req.to_expression(),
            req.peer_addr().to_expression(),
        ))
        .then(move |res| match res {
            // allow request
            Ok(Ok(Some(true))) => future::Either::A(
                payload
                    .from_err()
                    .fold(web::BytesMut::new(), |mut body, chunk| {
                        body.extend_from_slice(&chunk);
                        Ok::<_, Error>(body)
                    })
                    .and_then(move |client_payload| {
                        policy
                            .send(policy::Evaluate::ClientPayload(
                                client_payload.to_expression(),
                            ))
                            .then(move |res| {
                                match res {
                                    // allow payload
                                    Ok(Ok(Some(true))) | Ok(Ok(None)) => future::Either::A(
                                        req.forward_url(*proxy_port).and_then(move |url| {
                                            client
                                                .request_from(url.as_str(), req.head())
                                                .process_headers(req.peer_addr())
                                                // forward the request (with the original client payload)
                                                .send_body(client_payload)
                                                // send the response back to the client
                                                .then(|res| response(policy, res))
                                        }),
                                    ),
                                    // reject
                                    Ok(Ok(Some(false))) => future::Either::B(future::ok(
                                        unauthorized("request denied (bad client payload)"),
                                    )),
                                    // policy error
                                    Ok(Err(e)) => {
                                        warn!("{}", e);
                                        future::Either::B(future::ok(internal()))
                                    }
                                    // actor error
                                    Err(e) => {
                                        warn!("{}", e);
                                        future::Either::B(future::ok(internal()))
                                    }
                                }
                            })
                    }),
            ),
            // reject
            Ok(Ok(Some(false))) | Ok(Ok(None)) => {
                future::Either::B(future::ok(unauthorized("request denied")))
            }
            // policy error
            Ok(Err(e)) => {
                warn!("{}", e);
                future::Either::B(future::ok(internal()))
            }
            // actor error
            Err(e) => {
                warn!("{}", e);
                future::Either::B(future::ok(internal()))
            }
        })
}

/// Send server response back to client
pub fn response(
    policy: web::Data<actix::Addr<policy::DataPolicy>>,
    res: Result<
        ClientResponse<impl Stream<Item = web::Bytes, Error = client::PayloadError>>,
        SendRequestError,
    >,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match res {
        Ok(res) => {
            let mut client_resp = HttpResponse::build(res.status());
            for (header_name, header_value) in res
                .headers()
                .iter()
                .filter(|(h, _)| *h != "connection" && *h != "content-encoding")
            {
                // debug!("header {}: {:?}", header_name, header_value);
                client_resp.header(header_name.clone(), header_value.clone());
            }
            // get the server payload
            future::Either::A(
                res.from_err()
                    .fold(web::BytesMut::new(), |mut body, chunk| {
                        body.extend_from_slice(&chunk);
                        Ok::<_, Error>(body)
                    })
                    .and_then(move |server_payload| {
                        // debug!("{:?}", server_payload);
                        policy
                            .send(policy::Evaluate::ServerPayload(
                                server_payload.to_expression(),
                            ))
                            .then(move |res| match res {
                                // allow
                                Ok(Ok(Some(true))) | Ok(Ok(None)) => {
                                    future::ok(client_resp.body(server_payload))
                                }
                                // reject
                                Ok(Ok(Some(false))) => future::ok(
                                    HttpResponse::Unauthorized()
                                        .body("request denied (bad server payload)"),
                                ),
                                // policy error
                                Ok(Err(e)) => {
                                    warn!("{}", e);
                                    future::ok(internal())
                                }
                                // actor error
                                Err(e) => {
                                    warn!("{}", e);
                                    future::ok(internal())
                                }
                            })
                    }),
            )
        }
        // error response when connecting to server
        Err(err) => future::Either::B(future::ok(err.error_response())),
    }
}

fn internal() -> HttpResponse {
    HttpResponse::InternalServerError().body("Armour internal error")
}

fn unauthorized(message: &'static str) -> HttpResponse {
    HttpResponse::Unauthorized().body(message)
}

/// Extract a forwarding URL
trait ForwardUrl {
    fn forward_url(&self, proxy_port: u16) -> Box<dyn Future<Item = url::Url, Error = Error>>
    where
        Self: Sized;
}

// Get forwarding address from headers
impl ForwardUrl for HttpRequest {
    fn forward_url(&self, proxy_port: u16) -> Box<dyn Future<Item = url::Url, Error = Error>> {
        let info = self.connection_info();
        match url::Url::parse(&format!(
            "{}://{}{}",
            info.scheme(),
            info.host(),
            self.uri()
        )) {
            Ok(url) => {
                if url.port().unwrap_or(80) == proxy_port
                    && url.host().map(is_local_host).unwrap_or(true)
                {
                    warn!("trying to proxy self");
                    Box::new(future::err("cannot proxy self".to_actix()))
                } else {
                    info!("forward URL is: {}", url);
                    Box::new(future::ok(url))
                }
            }
            Err(err) => Box::new(future::err(err.to_actix())),
        }
    }
}

fn is_local_host(host: url::Host<&str>) -> bool {
    match host {
        url::Host::Domain("localhost") => true,
        url::Host::Ipv4(v4) => {
            v4.is_unspecified() || v4.is_broadcast() || v4 == std::net::Ipv4Addr::LOCALHOST
        }
        url::Host::Ipv6(v6) => v6.is_unspecified() || v6 == std::net::Ipv6Addr::LOCALHOST,
        _ => false,
    }
}

/// Conditionally set the `x-forwarded-for` header to be a TCP socket address
trait ProcessHeaders {
    fn process_headers(self, peer_addr: Option<std::net::SocketAddr>) -> Self;
}

impl ProcessHeaders for ClientRequest {
    fn process_headers(self, peer_addr: Option<std::net::SocketAddr>) -> Self {
        let mut req;
        if let Some(addr) = peer_addr {
            req = self.header("x-forwarded-for", format!("{}", addr))
        } else {
            req = self
        };
        if let Some(host) = req.headers().get("x-forwarded-host").cloned() {
            let headers = req.headers_mut();
            headers.remove("x-forwarded-host");
            headers.insert(
                HeaderName::from_static("host"),
                HeaderValue::from_bytes(host.as_ref()).unwrap(),
            );
        }
        req
    }
}

/// Trait for converting errors into actix-web errors
pub trait ToActixError {
    fn to_actix(self) -> Error
    where
        Self: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        std::io::Error::new(std::io::ErrorKind::Other, self).into()
    }
}

impl ToActixError for url::ParseError {}
impl ToActixError for http::header::ToStrError {}
impl ToActixError for &str {}
