//! HTTP proxy with Armour policies
use super::policy::{self, AcceptRequest, ToActixError};
use actix_web::client::{Client, ClientRequest};
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
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
/// Checks request against Armour policy and, if accepted, forwards to [forward_url](ForwardUrl)
fn forward(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<policy::ArmourPolicy>,
    client: web::Data<Client>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    Box::new(policy.accept(&req).and_then(|accept| {
        info!("accept is: {}", accept);
        if accept {
            future::Either::A(req.forward_url().and_then(move |url| {
                client
                    .request_from(url.as_str(), req.head())
                    .set_x_forward_for(req.peer_addr())
                    .send_stream(payload)
                    .from_err()
                    .map(|res| {
                        let mut client_resp = HttpResponse::build(res.status());
                        for (header_name, header_value) in
                            res.headers().iter().filter(|(h, _)| *h != "connection")
                        {
                            client_resp.header(header_name.clone(), header_value.clone());
                        }
                        client_resp.streaming(res)
                    })
            }))
        } else {
            future::Either::B(future::ok(
                HttpResponse::BadRequest().body("request denied"),
            ))
        }
    }))
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

/// Set the x-forwarded-for header to be the client's socket address
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

