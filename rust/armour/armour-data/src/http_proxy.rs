//! HTTP proxy with Armour policies

use super::http_policy::{EvalRestFn, GetRestPolicy, PolicyStatus, RestFn, RestPolicyResponse};
use super::policy::{PolicyActor, ID};
use super::ToArmourExpression;
use actix_web::{
    client::{Client, ClientRequest, ClientResponse, PayloadError, SendRequestError},
    dev::HttpResponseBuilder,
    http::header::{ContentEncoding, HeaderName, HeaderValue},
    http::uri,
    middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use armour_data_interface::own_ip;
use armour_policy::{lang::Policy, literals};
use futures::{future, stream::Stream, Future};
use std::collections::HashSet;

pub fn start_proxy(
    policy: actix::Addr<PolicyActor>,
    port: u16,
) -> std::io::Result<actix_web::dev::Server> {
    let socket_address = format!("0.0.0.0:{}", port);
    let server = HttpServer::new(move || {
        App::new()
            .data(policy.clone())
            .data(Client::new())
            .data(port)
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::new(ContentEncoding::Identity))
            .default_service(web::route().to_async(request))
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
fn request(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
    proxy_port: web::Data<u16>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    if let Ok(connection) = Connection::new(&req, *proxy_port) {
        future::Either::A(
            policy
                .send(GetRestPolicy(connection.from_to()))
                .then(move |p| {
                    if let Ok(p) = p {
                        // we succeeded in getting a policy
                        future::Either::A(match p.status {
                            // check request
                            PolicyStatus {
                                request: Policy::Args(count),
                                ..
                            } => {
                                let args = if count == 0 {
                                    vec![]
                                } else {
                                    vec![(&req, &p.connection).to_expression()]
                                };
                                let message = EvalRestFn(RestFn::Request, args);
                                future::Either::A(policy.send(message).then(move |allowed| {
                                    match allowed {
                                        // allow request
                                        Ok(Ok(true)) => future::Either::A(client_payload(
                                            p, req, connection, payload, policy, client,
                                        )),
                                        // reject
                                        Ok(Ok(false)) => future::Either::B(future::ok(
                                            unauthorized("bad client request"),
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
                                }))
                            }
                            // allow
                            PolicyStatus {
                                request: Policy::Allow,
                                ..
                            } => future::Either::B(future::Either::A(client_payload(
                                p, req, connection, payload, policy, client,
                            ))),
                            // deny
                            PolicyStatus {
                                request: Policy::Deny,
                                ..
                            } => future::Either::B(future::Either::B(future::ok(unauthorized(
                                "request denied",
                            )))),
                            // cannot be Unit policy
                            _ => unreachable!(),
                        })
                    } else {
                        // we failed to get a policy
                        warn!("{}", p.err().unwrap());
                        future::Either::B(future::ok(internal()))
                    }
                }),
        )
    } else {
        // could not obtain forwarding URL
        future::Either::B(future::ok(internal()))
    }
}

// Process request (so far it's allow by the policy)
fn client_payload(
    p: RestPolicyResponse,
    req: HttpRequest,
    connection: Connection,
    payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match p.status {
        // check client payload
        PolicyStatus {
            client_payload: Policy::Args(_arg_count),
            debug,
            timeout,
            // connection_number,
            ..
        } => future::Either::A(future::Either::B(
            payload
                .from_err()
                .fold(web::BytesMut::new(), |mut body, chunk| {
                    body.extend_from_slice(&chunk);
                    Ok::<_, Error>(body)
                })
                .and_then(move |client_payload| {
                    let payload = literals::Payload::from((client_payload.as_ref(), &p.connection));
                    let args = vec![payload.into()];
                    let message = EvalRestFn(RestFn::ClientPayload, args);
                    policy.send(message).then(move |allowed| {
                        match allowed {
                            // allow payload
                            Ok(Ok(true)) => {
                                let client_request = client
                                    .request_from(connection.uri(), req.head())
                                    .process_headers(req.peer_addr())
                                    .timeout(timeout);
                                if debug {
                                    debug!("{:?}", client_request)
                                };
                                future::Either::A(
                                    client_request
                                        // forward the request (with the original client payload)
                                        .send_body(client_payload)
                                        // send the response back to the client
                                        .then(move |res| response(p, policy, res)),
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
        )),
        // allow
        PolicyStatus {
            client_payload: Policy::Allow,
            debug,
            timeout,
            ..
        } => {
            let client_request = client
                .request_from(connection.uri(), req.head())
                .process_headers(req.peer_addr())
                .timeout(timeout);
            // let server_payload = fns.map(|x| x.server_payload).unwrap_or(false);
            if debug {
                debug!("{:?}", client_request)
            };
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
                            .then(move |res| response(p, policy, res))
                    }),
            )
        }
        // deny
        PolicyStatus {
            client_payload: Policy::Deny,
            ..
        } => future::Either::A(future::Either::A(future::ok(unauthorized(
            "request denied (bad client payload)",
        )))),
        // cannot be Unit policy
        _ => unreachable!(),
    }
}

/// Send server response back to client
fn response(
    p: RestPolicyResponse,
    policy: web::Data<actix::Addr<PolicyActor>>,
    res: Result<
        ClientResponse<impl Stream<Item = web::Bytes, Error = PayloadError> + 'static>,
        SendRequestError,
    >,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match res {
        Ok(res) => {
            let mut response_builder = HttpResponse::build(res.status());
            for (header_name, header_value) in res.headers().iter().filter(|(h, _)| {
                *h != "connection" && *h != "content-length" && *h != "content-encoding"
            }) {
                // debug!("header {}: {:?}", header_name, header_value);
                response_builder.header(header_name.clone(), header_value.clone());
            }
            let response: HttpResponse = response_builder.into();
            if p.status.debug {
                debug!("{:?}", response)
            }
            match p.status {
                // check server response
                PolicyStatus {
                    response: Policy::Args(count),
                    ..
                } => {
                    let args = if count == 0 {
                        vec![]
                    } else {
                        vec![(&response, &p.connection).to_expression()]
                    };
                    let message = EvalRestFn(RestFn::Response, args);
                    future::Either::A(policy.send(message).then(move |allowed| match allowed {
                        // allow
                        Ok(Ok(true)) => future::Either::A(server_payload(p, policy, response, res)),
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
                    }))
                }
                // allow
                PolicyStatus {
                    response: Policy::Allow,
                    ..
                } => future::Either::B(future::Either::A(server_payload(p, policy, response, res))),
                // deny
                PolicyStatus {
                    response: Policy::Deny,
                    ..
                } => future::Either::B(future::Either::B(future::ok(unauthorized(
                    "request denied (bad server response)",
                )))),
                // cannot be Unit policy
                _ => unreachable!(),
            }
        }
        // error response when connecting to server
        Err(err) => future::Either::B(future::Either::B(future::ok(err.error_response()))),
    }
}

/// Send server response back to client
fn server_payload(
    p: RestPolicyResponse,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client_resp: HttpResponse,
    res: ClientResponse<impl Stream<Item = web::Bytes, Error = PayloadError> + 'static>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    match p.status {
        // check server payload
        PolicyStatus {
            server_payload: Policy::Args(_arg_count),
            // connection_number,
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
                        // if debug {
                        //     debug!("\n{:?}", server_payload)
                        // };
                        let payload =
                            literals::Payload::from((server_payload.as_ref(), &p.connection));
                        let args = vec![payload.into()];
                        let message = EvalRestFn(RestFn::ServerPayload, args);
                        policy.send(message).then(move |allowed| match allowed {
                            // allow
                            // Ok(Ok(true)) => future::Either::A(client_resp.body(server_payload)),
                            Ok(Ok(true)) => future::Either::A(future::ok(
                                HttpResponseBuilder::from(client_resp).body(server_payload),
                            )),
                            // reject
                            Ok(Ok(false)) => future::Either::B(future::ok(unauthorized(
                                "request denied (bad server payload)",
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
                        })
                    }),
            )
        }
        // allow
        PolicyStatus {
            server_payload: Policy::Allow,
            ..
        } => future::Either::B(future::Either::B(
            res.from_err()
                .fold(web::BytesMut::new(), |mut body, chunk| {
                    body.extend_from_slice(&chunk);
                    Ok::<_, Error>(body)
                })
                .and_then(move |server_payload| {
                    future::ok(HttpResponseBuilder::from(client_resp).body(server_payload))
                }),
        )),
        // deny
        PolicyStatus {
            server_payload: Policy::Deny,
            ..
        } => future::Either::B(future::Either::A(future::ok(unauthorized(
            "request denied (bad server payload)",
        )))),
        // cannot be Unit policy
        _ => unreachable!(),
    }
}

fn internal() -> HttpResponse {
    HttpResponse::InternalServerError().body("Armour internal error")
}

fn unauthorized(message: &'static str) -> HttpResponse {
    HttpResponse::Unauthorized().body(message)
}

struct Connection {
    uri: uri::Uri,
    from: ID,
    to: ID,
}

impl Connection {
    fn new(req: &HttpRequest, proxy_port: u16) -> Result<Connection, ()> {
        // obtain the forwarding URI
        let uri = match req.forward_uri(proxy_port) {
            Ok(uri) => uri,
            Err(err) => {
                warn!("{}", err);
                return Err(());
            }
        };
        let to = uri.clone().into();
        Ok(Connection {
            uri,
            from: req.peer_addr().into(),
            to,
        })
    }
    fn uri(&self) -> &uri::Uri {
        &self.uri
    }
    fn from_to(&self) -> (ID, ID) {
        (self.from.clone(), self.to.clone())
    }
}

/// Extract a forwarding URL
trait ForwardUri {
    fn forward_uri(&self, proxy_port: u16) -> Result<uri::Uri, Error>
    where
        Self: Sized;
}

// Get forwarding address from headers
impl ForwardUri for HttpRequest {
    fn forward_uri(&self, proxy_port: u16) -> Result<uri::Uri, Error> {
        let info = self.connection_info();
        let mut uri = uri::Builder::new();
        uri.scheme(info.scheme());
        uri.authority(info.host());
        if let Some(p_and_q) = self.uri().path_and_query() {
            uri.path_and_query(p_and_q.clone());
        }
        match uri.build() {
            Ok(uri) => {
                if uri.port_u16().unwrap_or(80) == proxy_port
                    && uri.host().map(is_local_host).unwrap_or(true)
                {
                    warn!("trying to proxy self");
                    Err("cannot proxy self".to_actix())
                } else {
                    Ok(uri)
                }
            }
            Err(err) => Err(err.into()),
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

fn is_local_host(host: &str) -> bool {
    use std::str::FromStr;
    if let Ok(ipv4) = std::net::Ipv4Addr::from_str(host) {
        own_ip(&ipv4.into())
    } else {
        LOCAL_HOST_NAMES.contains(&host.to_ascii_lowercase())
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

impl ToActixError for http::header::ToStrError {}
impl ToActixError for &str {}
