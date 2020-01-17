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
use armour_lang::{lang::Policy, literals};
use armour_utils::own_ip;
use bytes::BytesMut;
use futures::{stream::Stream, StreamExt};
use std::collections::HashSet;

pub async fn start_proxy(
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
            .default_service(web::route().to(request))
    })
    .bind(&socket_address)?
    .run();
    log::info!("starting proxy server: http://{}", socket_address);
    Ok(server)
}

/// Main HttpRequest proxy
///
/// Checks request against Armour policy and, if accepted, forwards it using [ForwardUrl](trait.ForwardUrl.html).
/// The server response is then checked before it is forwarded back to the original client.
async fn request(
    req: HttpRequest,
    payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
    proxy_port: web::Data<u16>,
) -> Result<HttpResponse, Error> {
    if let Some(connection) = Connection::new(&req, **proxy_port) {
        if let Ok(p) = policy.send(GetRestPolicy(connection.from_to())).await {
            // we succeeded in getting a policy
            match p.status {
                // allow all
                PolicyStatus {
                    allow_all: true,
                    debug,
                    timeout,
                    ..
                } => allow_all(req, connection, payload, client, debug, timeout).await,
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
                    let allowed = policy.send(message).await;
                    match allowed {
                        // allow request
                        Ok(Ok(true)) => {
                            client_payload(p, req, connection, payload, policy, client).await
                        }
                        // reject
                        Ok(Ok(false)) => Ok(unauthorized("bad client request")),
                        // policy error
                        Ok(Err(e)) => {
                            warn!("{}", e);
                            Ok(internal())
                        }
                        // actor error
                        Err(e) => {
                            warn!("{}", e);
                            Ok(internal())
                        }
                    }
                }
                // allow
                PolicyStatus {
                    request: Policy::Allow,
                    ..
                } => client_payload(p, req, connection, payload, policy, client).await,
                // deny
                PolicyStatus {
                    request: Policy::Deny,
                    ..
                } => Ok(unauthorized("request denied")),
                // cannot be Unit policy
                _ => unreachable!(),
            }
        } else {
            // we failed to get a policy
            warn!("failed to get HTTP policy");
            Ok(internal())
        }
    } else {
        // could not obtain forwarding URL
        Ok(internal())
    }
}

// Streamlined processing of "allow all" policy
async fn allow_all(
    req: HttpRequest,
    connection: Connection,
    mut payload: web::Payload,
    client: web::Data<Client>,
    debug: bool,
    timeout: std::time::Duration,
) -> Result<HttpResponse, Error> {
    let mut client_payload = BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        client_payload.extend_from_slice(&chunk)
    }
    let client_request = client
        .request_from(connection.uri(), req.head())
        .process_headers(req.peer_addr())
        .timeout(timeout);
    if debug {
        debug!("{:?}", client_request)
    };
    // forward the request (with the original client payload)
    match client_request.send_body(client_payload).await {
        Ok(mut res) => {
            let mut server_payload = BytesMut::new();
            while let Some(chunk) = res.next().await {
                let chunk = chunk?;
                server_payload.extend_from_slice(&chunk)
            }
            let mut response_builder = HttpResponse::build(res.status());
            for (header_name, header_value) in res.headers().iter().filter(|(h, _)| {
                *h != "connection" && *h != "content-length" && *h != "content-encoding"
            }) {
                response_builder.header(header_name.clone(), header_value.clone());
            }
            let response = response_builder.body(server_payload);
            if debug {
                debug!("{:?}", response)
            }
            Ok(response)
        }
        Err(err) => Ok(err.error_response()),
    }
}

// Process request (so far it's allow by the policy)
async fn client_payload(
    p: RestPolicyResponse,
    req: HttpRequest,
    connection: Connection,
    mut payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
) -> Result<HttpResponse, Error> {
    match p.status {
        // check client payload
        PolicyStatus {
            client_payload: Policy::Args(_arg_count),
            debug,
            timeout,
            // connection_number,
            ..
        } => {
            let mut client_payload = BytesMut::new();
            while let Some(chunk) = payload.next().await {
                let chunk = chunk?;
                client_payload.extend_from_slice(&chunk)
            }
            let payload = literals::Payload::from((client_payload.as_ref(), &p.connection));
            let allowed = policy
                .send(EvalRestFn(RestFn::ClientPayload, vec![payload.into()]))
                .await;
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
                    // forward the request (with the original client payload)
                    let res = client_request.send_body(client_payload).await;
                    // send the response back to the client
                    response(p, policy, res).await
                }
                // reject
                Ok(Ok(false)) => Ok(unauthorized("request denied (bad client payload)")),
                // policy error
                Ok(Err(e)) => {
                    warn!("{}", e);
                    Ok(internal())
                }
                // actor error
                Err(e) => {
                    warn!("{}", e);
                    Ok(internal())
                }
            }
        }
        // allow
        PolicyStatus {
            client_payload: Policy::Allow,
            debug,
            timeout,
            ..
        } => {
            let mut client_payload = BytesMut::new();
            while let Some(chunk) = payload.next().await {
                let chunk = chunk?;
                client_payload.extend_from_slice(&chunk)
            }
            let client_request = client
                .request_from(connection.uri(), req.head())
                .process_headers(req.peer_addr())
                .timeout(timeout);
            if debug {
                debug!("{:?}", client_request)
            };
            // forward the request (with the original client payload)
            let res = client_request.send_body(client_payload).await;
            // send the response back to the client
            response(p, policy, res).await
        }
        // deny
        PolicyStatus {
            client_payload: Policy::Deny,
            ..
        } => Ok(unauthorized("request denied (bad client payload)")),
        // cannot be Unit policy
        _ => unreachable!(),
    }
}

/// Send server response back to client
async fn response(
    p: RestPolicyResponse,
    policy: web::Data<actix::Addr<PolicyActor>>,
    res: Result<
        ClientResponse<impl Stream<Item = Result<web::Bytes, PayloadError>> + Unpin>,
        SendRequestError,
    >,
) -> Result<HttpResponse, Error> {
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
                    let allowed = policy.send(message).await;
                    match allowed {
                        // allow
                        Ok(Ok(true)) => server_payload(p, policy, response, res).await,
                        // reject
                        Ok(Ok(false)) => Ok(unauthorized("request denied (bad server response)")),
                        // policy error
                        Ok(Err(e)) => {
                            warn!("{}", e);
                            Ok(internal())
                        }
                        // actor error
                        Err(e) => {
                            warn!("{}", e);
                            Ok(internal())
                        }
                    }
                }
                // allow
                PolicyStatus {
                    response: Policy::Allow,
                    ..
                } => server_payload(p, policy, response, res).await,
                // deny
                PolicyStatus {
                    response: Policy::Deny,
                    ..
                } => Ok(unauthorized("request denied (bad server response)")),
                // cannot be Unit policy
                _ => unreachable!(),
            }
        }
        // error response when connecting to server
        Err(err) => Ok(err.error_response()),
    }
}

/// Send server response back to client
async fn server_payload(
    p: RestPolicyResponse,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client_resp: HttpResponse,
    mut res: ClientResponse<impl Stream<Item = Result<web::Bytes, PayloadError>> + Unpin>,
) -> Result<HttpResponse, Error> {
    match p.status {
        // check server payload
        PolicyStatus {
            server_payload: Policy::Args(_arg_count),
            // connection_number,
            // debug,
            ..
        } => {
            // get the server payload
            let mut server_payload = BytesMut::new();
            while let Some(chunk) = res.next().await {
                let chunk = chunk?;
                server_payload.extend_from_slice(&chunk)
            }
            // if debug {
            //     debug!("\n{:?}", server_payload)
            // };
            let payload = literals::Payload::from((server_payload.as_ref(), &p.connection));
            let allowed = policy
                .send(EvalRestFn(RestFn::ServerPayload, vec![payload.into()]))
                .await;
            match allowed {
                // allow
                // Ok(Ok(true)) => future::Either::A(client_resp.body(server_payload)),
                Ok(Ok(true)) => Ok(HttpResponseBuilder::from(client_resp).body(server_payload)),
                // reject
                Ok(Ok(false)) => Ok(unauthorized("request denied (bad server payload)")),
                // policy error
                Ok(Err(e)) => {
                    warn!("{}", e);
                    Ok(internal())
                }
                // actor error
                Err(e) => {
                    warn!("{}", e);
                    Ok(internal())
                }
            }
        }
        // allow
        PolicyStatus {
            server_payload: Policy::Allow,
            ..
        } => {
            // get the server payload
            let mut server_payload = BytesMut::new();
            while let Some(chunk) = res.next().await {
                let chunk = chunk?;
                server_payload.extend_from_slice(&chunk)
            }
            Ok(HttpResponseBuilder::from(client_resp).body(server_payload))
        }
        // deny
        PolicyStatus {
            server_payload: Policy::Deny,
            ..
        } => Ok(unauthorized("request denied (bad server payload)")),
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
    fn new(req: &HttpRequest, proxy_port: u16) -> Option<Connection> {
        // obtain the forwarding URI
        match req.forward_uri(proxy_port) {
            Ok(uri) => {
                let to = uri.clone().into();
                Some(Connection {
                    uri,
                    from: req.peer_addr().into(),
                    to,
                })
            }
            Err(err) => {
                warn!("{}", err);
                None
            }
        }
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
        let mut uri = uri::Builder::new()
            .scheme(info.scheme())
            .authority(info.host());
        if let Some(p_and_q) = self.uri().path_and_query() {
            uri = uri.path_and_query(p_and_q.clone());
        }
        match uri.build() {
            Ok(uri) => {
                if uri.port_u16().unwrap_or(80) == proxy_port
                    && uri.host().map(is_local_host).unwrap_or(true)
                {
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
        for ip in armour_utils::INTERFACE_IPS.iter() {
            if let Ok(name) = dns_lookup::lookup_addr(ip) {
                names.insert(name);
            }
    // if let Ok(resolver) = trust_dns_resolver::Resolver::from_system_conf() {
    //         for ip in armour_api::INTERFACE_IPS.iter() {
    //             if let Ok(name) = dns_lookup::lookup_addr(ip) {
    //                 names.insert(name);
    //             }
    //             if let Ok(interface_names) = resolver.reverse_lookup(*ip) {
    //                 names.extend(
    //                     interface_names
    //                         .into_iter()
    //                         .map(|name| name.to_ascii().trim_end_matches('.').to_lowercase()),
    //                 )
    //             }
    //         }
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
