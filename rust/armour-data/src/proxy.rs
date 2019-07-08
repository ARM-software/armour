//! HTTP proxy with Armour policies

use super::policy::{self, ToArmourExpression};
use actix_web::client::{self, Client, ClientRequest, ClientResponse, SendRequestError};
use actix_web::{web, Error, HttpRequest, HttpResponse, ResponseError};
use futures::stream::Stream;
use futures::{future, Future};

/// Main HttpRequest proxy
///
/// Checks request against Armour policy and, if accepted, forwards it using [ForwardUrl](trait.ForwardUrl.html).
/// The server response is then checked before it is forwarded back to the original client.
pub fn proxy(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<actix::Addr<policy::DataPolicy>>,
    client: web::Data<Client>,
    address: web::Data<String>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    policy
        .send(policy::Evaluate::Require(req.to_expression(), req.peer_addr().to_expression()))
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
                                        req.forward_url(&address).and_then(move |url| {
                                            client
                                                .request_from(url.as_str(), req.head())
                                                .set_x_forward_for(req.peer_addr())
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
        Ok(mut res) => {
            let mut client_resp = HttpResponse::build(res.status());
            for (header_name, header_value) in
                res.headers().iter().filter(|(h, _)| *h != "connection")
            {
                client_resp.header(header_name.clone(), header_value.clone());
            }
            // get the server payload
            future::Either::A(res.body().from_err().and_then(move |server_payload| {
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
            }))
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
    fn forward_url(&self, proxy_address: &str) -> Box<dyn Future<Item = url::Url, Error = Error>>
    where
        Self: Sized;
}

// Get forwarding address from headers
impl ForwardUrl for HttpRequest {
    fn forward_url(&self, proxy_address: &str) -> Box<dyn Future<Item = url::Url, Error = Error>> {
        let info = self.connection_info();
        let host = info.host();
        if host == proxy_address {
            info!("trying to proxy self");
            Box::new(future::err("cannot proxy self".to_actix()))
        } else {
            let url_str = format!("{}://{}{}", info.scheme(), host, self.uri());
            match url::Url::parse(&url_str) {
                Ok(url) => {
                    info!("forward URL is: {}", url);
                    Box::new(future::ok(url))
                }
                Err(err) => Box::new(future::err(err.to_actix())),
            }
        }
    }
}

/// Conditionally set the `x-forwarded-for` header to be a TCP socket address
trait SetForwardFor {
    fn set_x_forward_for(self, a: Option<std::net::SocketAddr>) -> Self;
}

impl SetForwardFor for ClientRequest {
    fn set_x_forward_for(self, a: Option<std::net::SocketAddr>) -> Self {
        if let Some(addr) = a {
            self.header("x-forwarded-for", format!("{}", addr))
        } else {
            self
        }
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
