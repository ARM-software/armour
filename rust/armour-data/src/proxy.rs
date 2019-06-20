//! HTTP proxy with Armour policies

use super::policy::{self, EvaluatePolicy, ToActixError};
use actix_web::client::{self, Client, ClientRequest, ClientResponse, SendRequestError};
use actix_web::{
    middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use futures::stream::Stream;
use futures::{future, Future};

impl policy::ArmourPolicy {
    /// Start-up the proxy
    pub fn start<S: std::net::ToSocketAddrs + std::fmt::Display>(
        self,
        addr: S,
    ) -> std::io::Result<()> {
        let address = addr.to_string();
        info!("starting proxy server: http://{}", address);
        HttpServer::new(move || {
            App::new()
                .data(self.clone())
                .data(Client::new())
                .data(address.clone())
                .wrap(middleware::Logger::default())
                .default_service(web::route().to_async(proxy))
        })
        .bind(addr)?
        .system_exit()
        .run()
    }
}

/// Main HttpRequest handler
///
/// Checks request against Armour policy and, if accepted, forwards it using [ForwardUrl](trait.ForwardUrl.html).
/// The server reponse is then checked before it is forwarded back to the original client.
fn proxy(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<policy::ArmourPolicy>,
    client: web::Data<Client>,
    address: web::Data<String>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    policy
        .evaluate_policy("require", vec![&req])
        .and_then(|accept| {
            // check if request is allowed
            let accept = accept.unwrap_or(false);
            info!("accept: {}", accept);
            if accept {
                future::Either::A(
                    // read the payload
                    payload
                        .from_err()
                        .fold(web::BytesMut::new(), |mut body, chunk| {
                            body.extend_from_slice(&chunk);
                            Ok::<_, Error>(body)
                        })
                        .and_then(|client_payload| {
                            policy
                                .evaluate_policy("client_payload", vec![&client_payload])
                                .and_then(move |accept_client_payload| {
                                    // check if client payload is allowed
                                    let accept_client_payload =
                                        accept_client_payload.unwrap_or(true);
                                    info!("accept client payload: {}", accept_client_payload);
                                    if accept_client_payload {
                                        // get the forwardinng URL
                                        future::Either::A(req.forward_url(&address).and_then(
                                            move |url| {
                                                client
                                                    .request_from(url.as_str(), req.head())
                                                    .set_x_forward_for(req.peer_addr())
                                                    // forward the request (with the original client payload)
                                                    .send_body(client_payload)
                                                    // send the response back to the client
                                                    .then(|res| response(policy, res))
                                            },
                                        ))
                                    } else {
                                        future::Either::B(future::ok(
                                            HttpResponse::Unauthorized()
                                                .body("request denied (bad payload)"),
                                        ))
                                    }
                                })
                        }),
                )
            } else {
                future::Either::B(future::ok(
                    HttpResponse::Unauthorized().body("request denied"),
                ))
            }
        })
        .or_else(|_| future::ok(HttpResponse::InternalServerError().body("something went wrong")))
}

/// Send server response back to client
fn response(
    policy: web::Data<policy::ArmourPolicy>,
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
                    .evaluate_policy("server_payload", vec![&server_payload])
                    .map(move |accept_server_payload| {
                        // check that the server payload is allowed
                        let accept_server_payload = accept_server_payload.unwrap_or(true);
                        info!("accept server payload: {}", accept_server_payload);
                        if accept_server_payload {
                            // send the server response back to the client
                            client_resp.body(server_payload)
                        } else {
                            HttpResponse::Unauthorized().body("request denied (bad response)")
                        }
                    })
            }))
        }
        Err(err) => future::Either::B(future::ok(err.error_response())),
    }
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
        let host = self.headers().get("ForwardTo").map_or_else(
            || {
                let s = info.host();
                if s == proxy_address {
                    info!("trying to proxy self");
                    Err("cannot proxy self".to_actix())
                } else {
                    Ok(s)
                }
            },
            |url| url.to_str().map_err(|e| e.to_actix()),
        );
        match host {
            Ok(host) => {
                let url_str = format!("{}://{}{}", info.scheme(), host, self.uri());
                match url::Url::parse(&url_str) {
                    Ok(url) => {
                        info!("forward URL is: {}", url);
                        Box::new(future::ok(url))
                    }
                    Err(err) => Box::new(future::err(err.to_actix())),
                }
            }
            Err(err) => Box::new(future::err(err)),
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
