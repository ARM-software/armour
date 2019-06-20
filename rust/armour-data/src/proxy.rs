//! HTTP proxy with Armour policies
use super::policy::{self, EvaluatePolicy, ToActixError};
use actix_web::client::{Client, ClientRequest};
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};

use futures::stream::Stream;
use futures::{future, Future};
/// Start-up the proxy
pub fn start<S: std::net::ToSocketAddrs + std::fmt::Display>(
    policy: policy::ArmourPolicy,
    addr: S,
) -> std::io::Result<()> {
    info!("starting proxy server: http://{}", addr);
    HttpServer::new(move || {
        App::new()
            .data(policy.clone())
            .data(Client::new())
            .wrap(middleware::Logger::default())
            .default_service(web::route().to_async(forward))
    })
    .bind(addr)?
    .system_exit()
    .run()
}

/// Main HttpRequest handler
///
/// Checks request against Armour policy and, if accepted, forwards it using [ForwardUrl](trait.ForwardUrl.html).
/// The server reponse is then checked before it is forwarded back to the original client.
fn forward(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<policy::ArmourPolicy>,
    client: web::Data<Client>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    policy.evaluate_policy("require", vec![&req]).and_then(|accept| {
        // check if request is allowed
        let accept = accept.unwrap_or(false);
        info!("accept: {}", accept);
        if accept {
            future::Either::A(
                // read the payload
                payload
                    .map_err(Error::from)
                    .fold(web::BytesMut::new(), |mut body, chunk| {
                        body.extend_from_slice(&chunk);
                        Ok::<_, Error>(body)
                    })
                    .and_then(|client_payload| {
                        policy
                            .evaluate_policy("client_payload", vec![&client_payload])
                            .and_then(move |accept_client_payload| {
                                // check if client payload is allowed
                                let accept_client_payload = accept_client_payload.unwrap_or(true);
                                info!("accept client payload: {}", accept_client_payload);
                                if accept_client_payload {
                                    // get the forwardinng URL
                                    future::Either::A(req.forward_url().and_then(move |url| {
                                        client
                                            .request_from(url.as_str(), req.head())
                                            .set_x_forward_for(req.peer_addr())
                                            // forward the request (with the original client payload)
                                            .send_body(client_payload)
                                            .from_err()
                                            .and_then(|mut res| {
                                                let mut client_resp =
                                                    HttpResponse::build(res.status());
                                                for (header_name, header_value) in res
                                                    .headers()
                                                    .iter()
                                                    .filter(|(h, _)| *h != "connection")
                                                {
                                                    client_resp.header(
                                                        header_name.clone(),
                                                        header_value.clone(),
                                                    );
                                                }
                                                // get the server payload
                                                res.body().from_err().and_then(
                                                    move |server_payload| {
                                                        policy
                                                            .evaluate_policy(
                                                                "server_payload",
                                                                vec![&server_payload],
                                                            )
                                                            .map(move |accept_server_payload| {
                                                                // check that the server payload is allowed
                                                                let accept_server_payload = accept_server_payload.unwrap_or(true);
                                                                info!(
                                                                    "accept server payload: {}",
                                                                    accept_server_payload
                                                                );
                                                                if accept_server_payload {
                                                                    // send the server response back to the client
                                                                    client_resp.body(server_payload)
                                                                } else {
                                                                    HttpResponse::BadRequest()
                                                                        .body("request denied (bad response)")
                                                                }
                                                            })
                                                    },
                                                )
                                            })
                                    }))
                                } else {
                                    future::Either::B(future::ok(
                                        HttpResponse::BadRequest().body("request denied (bad payload)"),
                                    ))
                                }
                            })
                    }),
            )
        } else {
            future::Either::B(future::ok(
                HttpResponse::BadRequest().body("request denied"),
            ))
        }
    })
}

/// Extract a forwarding URL
trait ForwardUrl {
    fn forward_url(&self) -> Box<dyn Future<Item = url::Url, Error = Error>>
    where
        Self: Sized;
}

// Get forwarding address from request information
// impl ForwardUrl for HttpRequest {
//     fn forward_url(&self) -> Box<dyn Future<Item = url::Url, Error = Error>> {
//         let info = self.connection_info();
//         match url::Url::parse(&format!(
//             "{}://{}{}",
//             info.scheme(),
//             info.host(),
//             self.uri()
//         )) {
//             Ok(url) => Box::new({
//                 info!("forward url is: {}", url);
//                 future::ok(url)
//             }),
//             Err(err) => Box::new(future::err(Error::from(std::io::Error::new(
//                 std::io::ErrorKind::Other,
//                 err,
//             )))),
//         }
//     }
// }

// Get forwarding address from headers
impl ForwardUrl for HttpRequest {
    fn forward_url(&self) -> Box<dyn Future<Item = url::Url, Error = Error>> {
        match self.headers().get("ForwardTo") {
            Some(url) => match url.to_str().map(|host| {
                format!(
                    "{}://{}{}",
                    self.connection_info().scheme(),
                    host,
                    self.uri()
                )
                .parse()
            }) {
                Ok(Ok(url)) => {
                    debug!("forwarding to: {}", url);
                    Box::new(future::ok(url))
                }
                Ok(Err(err)) => Box::new(future::err(err.to_actix())),
                Err(err) => Box::new(future::err(err.to_actix())),
            },
            None => Box::new(future::err("ForwardTo header missing".to_actix())),
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

