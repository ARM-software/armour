//! HTTP proxy with Armour policies

use super::policy::{self, Policy, ToArmourExpression};
use actix_web::client::{self, Client, ClientRequest, ClientResponse, SendRequestError};
use actix_web::{
    http::header::{ContentEncoding, HeaderName, HeaderValue},
    middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use armour_data_interface::{own_ip, ProxyConfig};
use futures::stream::Stream;
use futures::{future, Future};
use std::collections::HashSet;
use std::net::IpAddr;

pub fn start_proxy(
    policy: actix::Addr<policy::DataPolicy>,
    config: armour_data_interface::ProxyConfig,
) -> std::io::Result<actix_web::dev::Server> {
    let socket_address = format!("0.0.0.0:{}", config.port);
    let encoding = if config.response_streaming {
        ContentEncoding::Auto
    } else {
        ContentEncoding::Identity
    };
    let server = HttpServer::new(move || {
        App::new()
            .data(policy.clone())
            .data(Client::new())
            .data(config.clone())
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::new(encoding))
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
    config: web::Data<ProxyConfig>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    policy.send(policy::Check).then(|p| {
        if let Ok(policy::Policy { debug: true, .. }) = p {
            debug!("{:?}", req)
        }
        match p {
            // reject request
            Ok(policy::Policy {
                fns:
                    Some(policy::PolicyFns {
                        require: None,
                        client_payload: false,
                        server_payload: false,
                    }),
                ..
            }) => future::Either::B(future::Either::A(future::ok(unauthorized(
                "request denied",
            )))),
            // check "require"
            Ok(policy::Policy {
                fns:
                    Some(policy::PolicyFns {
                        require: Some(args),
                        server_payload,
                        ..
                    }),
                ..
            }) => {
                match req.forward_url((*config).port) {
                    Ok(url) => {
                        let message = match args {
                            0 => policy::Evaluate::Require0,
                            1 => policy::Evaluate::Require1(req.to_expression()),
                            _ => policy::Evaluate::Require3(
                                Box::new(req.to_expression()),
                                Box::new(req.peer_addr().to_expression()),
                                Box::new(url.to_expression()),
                            ),
                        };
                        future::Either::A(policy.send(message).then(move |res| match res {
                            // allow request
                            Ok(Ok(true)) => future::Either::A({
                                let no_decompress = config.response_streaming && !server_payload;
                                request(
                                    p.unwrap(),
                                    req,
                                    url,
                                    payload,
                                    policy,
                                    client,
                                    config,
                                    no_decompress,
                                )
                            }),
                            // reject
                            Ok(Ok(false)) => {
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
                        }))
                    }
                    Err(e) => {
                        warn!("{}", e);
                        future::Either::B(future::Either::A(future::ok(internal())))
                    }
                }
            }
            // allow request
            Ok(policy::Policy { fns: None, .. }) => match req.forward_url((*config).port) {
                Ok(url) => {
                    let no_decompress = config.response_streaming;
                    future::Either::B(future::Either::B(request(
                        p.unwrap(),
                        req,
                        url,
                        payload,
                        policy,
                        client,
                        config,
                        no_decompress,
                    )))
                }
                Err(err) => {
                    warn!("{}", err);
                    future::Either::B(future::Either::A(future::ok(internal())))
                }
            },
            Ok(policy::Policy {
                fns:
                    Some(policy::PolicyFns {
                        require: None,
                        server_payload,
                        ..
                    }),
                ..
            }) => match req.forward_url((*config).port) {
                Ok(url) => {
                    let no_decompress = config.response_streaming && !server_payload;
                    future::Either::B(future::Either::B(request(
                        p.unwrap(),
                        req,
                        url,
                        payload,
                        policy,
                        client,
                        config,
                        no_decompress,
                    )))
                }
                Err(err) => {
                    warn!("{}", err);
                    future::Either::B(future::Either::A(future::ok(internal())))
                }
            },
            // actor error from sending message Check
            Err(err) => {
                warn!("{}", err);
                future::Either::B(future::Either::A(future::ok(internal())))
            }
        }
    })
}

// Process request (so far it's allow by the policy)
#[allow(clippy::too_many_arguments)]
pub fn request(
    p: Policy,
    req: HttpRequest,
    url: url::Url,
    payload: web::Payload,
    policy: web::Data<actix::Addr<policy::DataPolicy>>,
    client: web::Data<Client>,
    config: web::Data<ProxyConfig>,
    no_decompress: bool,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match p {
        // check client payload
        policy::Policy {
            fns:
                Some(policy::PolicyFns {
                    client_payload: true,
                    ..
                }),
            debug,
            timeout,
        } => future::Either::A(
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
                                Ok(Ok(true)) => {
                                    let mut client_request = client
                                        .request_from(url.as_str(), req.head())
                                        .process_headers(req.peer_addr())
                                        .timeout(timeout);
                                    if no_decompress {
                                        client_request = client_request.no_decompress()
                                    }
                                    if debug {
                                        debug!("{:?}", client_request)
                                    };
                                    future::Either::A(
                                        client_request
                                            // forward the request (with the original client payload)
                                            .send_body(client_payload)
                                            // send the response back to the client
                                            .then(move |res| {
                                                response(
                                                    p,
                                                    policy,
                                                    res,
                                                    config.response_streaming,
                                                    no_decompress,
                                                )
                                            }),
                                    )
                                }
                                // reject
                                Ok(Ok(false)) => future::Either::B(future::ok(unauthorized(
                                    "request denied (bad client payload)",
                                ))),
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
        // allow client payload without check
        policy::Policy { debug, timeout, .. } => {
            let mut client_request = client
                .request_from(url.as_str(), req.head())
                .process_headers(req.peer_addr())
                .timeout(timeout);
            // let server_payload = fns.map(|x| x.server_payload).unwrap_or(false);
            if no_decompress {
                client_request = client_request.no_decompress()
            }
            if debug {
                debug!("{:?}", client_request)
            };
            future::Either::B(if config.request_streaming {
                future::Either::A(
                    client_request
                        // forward the request (with the original client payload)
                        .send_stream(payload)
                        // send the response back to the client
                        .then(move |res| {
                            response(p, policy, res, config.response_streaming, no_decompress)
                        }),
                )
            } else {
                future::Either::B(
                    payload
                        .from_err()
                        .fold(web::BytesMut::new(), |mut body, chunk| {
                            body.extend_from_slice(&chunk);
                            Ok::<_, Error>(body)
                        })
                        .and_then(move |client_payload| {
                            client_request
                                // forward the request (with the original client payload)
                                .send_body(client_payload)
                                // send the response back to the client
                                .then(move |res| {
                                    response(
                                        p,
                                        policy,
                                        res,
                                        config.response_streaming,
                                        no_decompress,
                                    )
                                })
                        }),
                )
            })
        }
    }
}

/// Send server response back to client
pub fn response(
    p: Policy,
    policy: web::Data<actix::Addr<policy::DataPolicy>>,
    res: Result<
        ClientResponse<impl Stream<Item = web::Bytes, Error = client::PayloadError> + 'static>,
        SendRequestError,
    >,
    response_streaming: bool,
    no_decompress: bool,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match res {
        Ok(res) => {
            let mut client_resp = HttpResponse::build(res.status());
            for (header_name, header_value) in res.headers().iter().filter(|(h, _)| {
                *h != "connection"
                    && *h != "content-length"
                    && (no_decompress || *h != "content-encoding")
            }) {
                // debug!("header {}: {:?}", header_name, header_value);
                client_resp.header(header_name.clone(), header_value.clone());
            }
            if p.debug {
                debug!("{:?}", client_resp)
            }
            future::Either::A(match p {
                policy::Policy {
                    fns:
                        Some(policy::PolicyFns {
                            server_payload: true,
                            ..
                        }),
                    ..
                } => {
                    future::Either::A(
                        // get the server payload
                        res.from_err()
                            .fold(web::BytesMut::new(), |mut body, chunk| {
                                body.extend_from_slice(&chunk);
                                Ok::<_, Error>(body)
                            })
                            .and_then(move |server_payload| {
                                debug!("{:?}", server_payload);
                                policy
                                    .send(policy::Evaluate::ServerPayload(
                                        server_payload.to_expression(),
                                    ))
                                    .then(move |res| match res {
                                        // allow
                                        Ok(Ok(true)) => {
                                            future::ok(client_resp.body(server_payload))
                                        }
                                        // reject
                                        Ok(Ok(false)) => future::ok(unauthorized(
                                            "request denied (bad server payload)",
                                        )),
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
                _ => {
                    if response_streaming {
                        future::Either::B(future::Either::A(future::ok(client_resp.streaming(res))))
                    } else {
                        future::Either::B(future::Either::B(
                            res.from_err()
                                .fold(web::BytesMut::new(), |mut body, chunk| {
                                    body.extend_from_slice(&chunk);
                                    Ok::<_, Error>(body)
                                })
                                .and_then(move |server_payload| {
                                    future::ok(client_resp.body(server_payload))
                                }),
                        ))
                    }
                }
            })
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
    fn forward_url(&self, proxy_port: u16) -> Result<url::Url, Error>
    where
        Self: Sized;
}

// Get forwarding address from headers
impl ForwardUrl for HttpRequest {
    fn forward_url(&self, proxy_port: u16) -> Result<url::Url, Error> {
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
                    Err("cannot proxy self".to_actix())
                } else {
                    info!("forward URL is: {}", url);
                    Ok(url)
                }
            }
            Err(err) => Err(err.to_actix()),
        }
    }
}

lazy_static! {
    pub static ref LOCAL_HOST_NAMES: HashSet<String> = {
        let mut names = HashSet::new();
        if let Ok(resolver) = trust_dns_resolver::Resolver::from_system_conf() {
            for ip in armour_data_interface::INTERFACE_IPS.iter() {
                if let Ok(name) = dns_lookup::lookup_addr(ip) {
                    names.insert(name);
                }
                if let Ok(interface_names) = resolver.reverse_lookup(*ip) {
                    names.extend(
                        interface_names
                            .into_iter()
                            .map(|name| name.to_ascii().trim_end_matches('.').to_lowercase()),
                    )
                }
            }
        };
        names
    };
}

fn is_local_host(host: url::Host<&str>) -> bool {
    match host {
        url::Host::Domain(domain) => LOCAL_HOST_NAMES.contains(&domain.to_ascii_lowercase()),
        url::Host::Ipv4(v4) => own_ip(&IpAddr::V4(v4)),
        url::Host::Ipv6(v6) => own_ip(&IpAddr::V6(v6)),
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
