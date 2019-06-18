use super::policy::{self, HttpAccept};
use actix_web::client::{Client, ClientRequest};
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use futures::{future, Future};

pub fn start<S: std::net::ToSocketAddrs + std::fmt::Display>(
    state: policy::ArmourState,
    addr: S,
) -> std::io::Result<()> {
    info!("starting proxy server: http://{}", addr);
    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .data(Client::new())
            .wrap(middleware::Logger::default())
            .default_service(web::route().to_async(forward))
    })
    .bind(addr)?
    .system_exit()
    .run()
}

// Main HttpRequest handler: checks against Armour policy and, if accepted, forwards to "forward_url"
fn forward(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<policy::ArmourState>,
    client: web::Data<Client>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    Box::new(policy.get_ref().accept(&req).and_then(|accept| {
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

trait LiftError {
    fn lift(self) -> Error
    where
        Self: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Error::from(std::io::Error::new(std::io::ErrorKind::Other, self))
    }
}

impl LiftError for url::ParseError {}
impl LiftError for http::header::ToStrError {}
impl LiftError for &str {}

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
                Ok(Err(err)) => Box::new(future::err(err.lift())),
                Err(err) => Box::new(future::err(err.lift())),
            },
            None => Box::new(future::err("ForwardTo header missing".lift())),
        }
    }
}

trait SetForwardFor {
    fn set_x_forward_for(self, a: Option<std::net::SocketAddr>) -> Self;
}

impl SetForwardFor for ClientRequest {
    fn set_x_forward_for(self, a: Option<std::net::SocketAddr>) -> Self {
        if let Some(addr) = a {
            self.header("x-forwarded-for", format!("{}", addr.ip()))
        } else {
            self
        }
    }
}

