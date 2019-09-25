//! HTTP proxy with Armour policies

use super::http_policy::{EvalRestFn, GetRestPolicy, RestFn, RestPolicyStatus};
use super::policy::PolicyActor;
use super::ToArmourExpression;
use actix_web::{
    client::{Client, ClientRequest, ClientResponse, PayloadError, SendRequestError},
    http::header::{ContentEncoding, HeaderName, HeaderValue},
    middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use armour_data_interface::{own_ip, HttpConfig};
use armour_policy::expressions::Expr;
use futures::{future, stream::Stream, Future};
use std::collections::HashSet;
use std::net::IpAddr;

pub fn start_proxy(
    policy: actix::Addr<PolicyActor>,
    config: armour_data_interface::HttpConfig,
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
            .default_service(web::route().to_async(request))
    })
    .bind(&socket_address)?
    .start();
    log::info!("starting proxy server: http://{}", socket_address);
    Ok(server)
}

struct Connection {
    no_decompress: bool,
    url: url::Url,
    from: Expr,
    to: Expr,
}

impl Connection {
    fn new(p: &RestPolicyStatus, req: &HttpRequest, config: &HttpConfig) -> Result<Connection, ()> {
        // obtain the forwarding URL
        let url = match req.forward_url((*config).port) {
            Ok(url) => url,
            Err(err) => {
                warn!("{}", err);
                return Err(());
            }
        };
        // do not bother decompressing the server payload if streaming is allowed and we are not
        // checking the payload
        let no_decompress =
            (p.allow_all || p.server_payload.is_none()) && config.response_streaming;
        let (from, to) = match p {
            // the policy inpterpreter will be needing the endpoint IDs
            RestPolicyStatus {
                allow_all: false,
                request: Some(n),
                ..
            }
            | RestPolicyStatus {
                allow_all: false,
                client_payload: Some(n),
                ..
            }
            | RestPolicyStatus {
                allow_all: false,
                server_payload: Some(n),
                ..
            } if 3 <= *n => (req.peer_addr().to_expression(), url.clone().to_expression()),
            _ => (Expr::default(), Expr::default()),
        };
        if let Some(peer) = from.host() {
            info!(r#"request from "{}" to "{}""#, peer, url)
        }
        Ok(Connection {
            no_decompress,
            url,
            from,
            to,
        })
    }
    fn from(&self) -> Expr {
        self.from.clone()
    }
    fn to(&self) -> Expr {
        self.to.clone()
    }
    fn url(&self) -> &str {
        self.url.as_str()
    }
}

/// Main HttpRequest proxy
///
/// Checks request against Armour policy and, if accepted, forwards it using [ForwardUrl](trait.ForwardUrl.html).
/// The server response is then checked before it is forwarded back to the original client.
fn request(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
    config: web::Data<HttpConfig>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    policy.send(GetRestPolicy).then(|p| {
        if let Ok(RestPolicyStatus { debug: true, .. }) = p {
            debug!("{:?}", req)
        }
        match p {
            // we got a policy
            Ok(p) => {
                match Connection::new(&p, &req, &config) {
                    Ok(connection) => match p {
                        // "allow all" policy
                        RestPolicyStatus {
                            allow_all: true, ..
                        } => future::Either::B(future::Either::B(client_payload(
                            p, req, connection, payload, policy, client, config,
                        ))),
                        // deny all
                        RestPolicyStatus {
                            request: None,
                            client_payload: None,
                            server_payload: None,
                            ..
                        } => future::Either::B(future::Either::A(future::ok(unauthorized(
                            "request denied",
                        )))),
                        // check "require"
                        RestPolicyStatus {
                            request: Some(arg_count),
                            connection_number,
                            ..
                        } => {
                            let args = match arg_count {
                                0 => Vec::new(),
                                1 => vec![req.to_expression()],
                                3 => vec![req.to_expression(), connection.from(), connection.to()],
                                4 => vec![
                                    req.to_expression(),
                                    connection.from(),
                                    connection.to(),
                                    connection_number.to_expression(),
                                ],
                                _ => unreachable!(),
                            };
                            future::Either::A(policy.send(EvalRestFn(RestFn::Request, args)).then(
                                move |allowed| match allowed {
                                    // allow request
                                    Ok(Ok(true)) => future::Either::A({
                                        client_payload(
                                            p, req, connection, payload, policy, client, config,
                                        )
                                    }),
                                    // reject
                                    Ok(Ok(false)) => future::Either::B(future::ok(unauthorized(
                                        "bad client request",
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
                                },
                            ))
                        }
                        // no "require" function
                        RestPolicyStatus { request: None, .. } => {
                            future::Either::B(future::Either::B(client_payload(
                                p, req, connection, payload, policy, client, config,
                            )))
                        }
                    },
                    // could not obtain forwarding URL
                    Err(()) => {
                        if p.request.is_none()
                            && p.client_payload.is_none()
                            && p.server_payload.is_none()
                        {
                            future::Either::B(future::Either::A(future::ok(unauthorized(
                                "request denied",
                            ))))
                        } else {
                            future::Either::B(future::Either::A(future::ok(internal())))
                        }
                    }
                }
            }
            // we failed to get a policy
            Err(err) => {
                warn!("{}", err);
                future::Either::B(future::Either::A(future::ok(internal())))
            }
        }
    })
}

// Process request (so far it's allow by the policy)
fn client_payload(
    p: RestPolicyStatus,
    req: HttpRequest,
    connection: Connection,
    payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
    config: web::Data<HttpConfig>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match p {
        // check client payload
        RestPolicyStatus {
            allow_all: false,
            client_payload: Some(arg_count),
            debug,
            timeout,
            connection_number,
            ..
        } => future::Either::A(
            payload
                .from_err()
                .fold(web::BytesMut::new(), |mut body, chunk| {
                    body.extend_from_slice(&chunk);
                    Ok::<_, Error>(body)
                })
                .and_then(move |client_payload| {
                    let args = match arg_count {
                        1 => vec![client_payload.to_expression()],
                        3 => vec![
                            client_payload.to_expression(),
                            connection.from(),
                            connection.to(),
                        ],
                        4 => vec![
                            client_payload.to_expression(),
                            connection.from(),
                            connection.to(),
                            connection_number.to_expression(),
                        ],
                        _ => unreachable!(),
                    };
                    policy
                        .send(EvalRestFn(RestFn::ClientPayload, args))
                        .then(move |allowed| {
                            match allowed {
                                // allow payload
                                Ok(Ok(true)) => {
                                    let mut client_request = client
                                        .request_from(connection.url(), req.head())
                                        .process_headers(req.peer_addr())
                                        .timeout(timeout);
                                    if connection.no_decompress {
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
                                                server_payload(
                                                    p,
                                                    policy,
                                                    res,
                                                    config.response_streaming,
                                                    connection,
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
        RestPolicyStatus { debug, timeout, .. } => {
            let mut client_request = client
                .request_from(connection.url(), req.head())
                .process_headers(req.peer_addr())
                .timeout(timeout);
            // let server_payload = fns.map(|x| x.server_payload).unwrap_or(false);
            if connection.no_decompress {
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
                            server_payload(p, policy, res, config.response_streaming, connection)
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
                                    server_payload(
                                        p,
                                        policy,
                                        res,
                                        config.response_streaming,
                                        connection,
                                    )
                                })
                        }),
                )
            })
        }
    }
}

/// Send server response back to client
fn server_payload(
    p: RestPolicyStatus,
    policy: web::Data<actix::Addr<PolicyActor>>,
    res: Result<
        ClientResponse<impl Stream<Item = web::Bytes, Error = PayloadError> + 'static>,
        SendRequestError,
    >,
    response_streaming: bool,
    connection: Connection,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match res {
        Ok(res) => {
            let mut client_resp = HttpResponse::build(res.status());
            for (header_name, header_value) in res.headers().iter().filter(|(h, _)| {
                *h != "connection"
                    && *h != "content-length"
                    && (connection.no_decompress || *h != "content-encoding")
            }) {
                // debug!("header {}: {:?}", header_name, header_value);
                client_resp.header(header_name.clone(), header_value.clone());
            }
            if p.debug {
                debug!("{:?}", client_resp)
            }
            future::Either::A(match p {
                RestPolicyStatus {
                    allow_all: false,
                    server_payload: Some(arg_count),
                    connection_number,
                    // debug,
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
                                let args = match arg_count {
                                    1 => vec![server_payload.to_expression()],
                                    3 => vec![
                                        server_payload.to_expression(),
                                        connection.from(),
                                        connection.to(),
                                    ],
                                    4 => vec![
                                        server_payload.to_expression(),
                                        connection.from(),
                                        connection.to(),
                                        connection_number.to_expression(),
                                    ],
                                    _ => unreachable!(),
                                };
                                // if debug {
                                //     debug!("\n{:?}", server_payload)
                                // };
                                policy.send(EvalRestFn(RestFn::ServerPayload, args)).then(
                                    move |allowed| match allowed {
                                        // allow
                                        Ok(Ok(true)) => future::Either::A(response(
                                            p,
                                            client_resp.body(server_payload),
                                            policy,
                                            connection,
                                        )),
                                        // reject
                                        Ok(Ok(false)) => future::Either::B(future::ok(
                                            unauthorized("request denied (bad server payload)"),
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
                                    },
                                )
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
                                    response(
                                        p,
                                        client_resp.body(server_payload),
                                        policy,
                                        connection,
                                    )
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

/// Send server response back to client
fn response(
    p: RestPolicyStatus,
    res: HttpResponse,
    policy: web::Data<actix::Addr<PolicyActor>>,
    connection: Connection,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match p {
        // check client payload
        RestPolicyStatus {
            allow_all: false,
            response: Some(arg_count),
            connection_number,
            ..
        } => {
            let args = match arg_count {
                1 => vec![res.to_expression()],
                3 => vec![res.to_expression(), connection.from(), connection.to()],
                4 => vec![
                    res.to_expression(),
                    connection.from(),
                    connection.to(),
                    connection_number.to_expression(),
                ],
                _ => unreachable!(),
            };
            future::Either::A(policy.send(EvalRestFn(RestFn::Response, args)).then(
                move |allowed| match allowed {
                    // allow
                    Ok(Ok(true)) => future::Either::A(future::ok(res)),
                    // reject
                    Ok(Ok(false)) => future::Either::B(future::ok(unauthorized(
                        "request denied (bad server response)",
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
                },
            ))
        }
        _ => future::Either::B(future::ok(res)),
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
trait ToActixError {
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
