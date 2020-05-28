//! HTTP proxy with Armour policies

use super::http_policy::{EvalHttpFn, GetHttpPolicy, HttpFn, HttpPolicyResponse, PolicyStatus};
use super::policy::{PolicyActor, ID};
use super::ToArmourExpression;
use actix_web::{
    client::{Client, ClientRequest, ClientResponse, Connector, PayloadError, SendRequestError},
    http::header::{ContentEncoding, HeaderMap, HeaderName, HeaderValue},
    http::uri,
    middleware, web, App, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use armour_lang::policies::FnPolicy;
use armour_utils::own_ip;
use bytes::BytesMut;
use futures::{stream::Stream, StreamExt};
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::convert::TryFrom;

pub async fn start_proxy(
    policy: actix::Addr<PolicyActor>,
    port: u16,
) -> std::io::Result<actix_web::dev::Server> {
    let socket_address = format!("0.0.0.0:{}", port);
    let server = HttpServer::new(move || {
        let config = actix_connect::resolver::ResolverConfig::default();
        let mut opts = actix_connect::resolver::ResolverOpts::default();
        opts.use_hosts_file = true;
        let resolver = actix_connect::start_resolver(config, opts);
        let connector = Connector::new()
            .connector(actix_connect::new_connector(resolver))
            .finish();
        let client = Client::build().connector(connector).finish();
        App::new()
            .data(policy.clone())
            .data(client)
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
    mut payload: web::Payload,
    policy: web::Data<actix::Addr<PolicyActor>>,
    client: web::Data<Client>,
    proxy_port: web::Data<u16>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(connection) = Connection::new(&req, **proxy_port) {
        if let Ok(p) = policy.send(GetHttpPolicy(connection.from_to())).await {
            // we succeeded in getting a policy
            match p.status {
                // check request
                PolicyStatus {
                    request: FnPolicy::Args(count),
                    timeout,
                    ..
                } => {
                    log::debug!("{:?}", req);
                    let mut client_payload = BytesMut::new();
                    while let Some(chunk) = payload.next().await {
                        let chunk = chunk?;
                        client_payload.extend_from_slice(&chunk)
                    }
                    let args = match count {
                        0 => vec![],
                        1 => vec![(&req, &p.connection).to_expression()],
                        2 => vec![
                            (&req, &p.connection).to_expression(),
                            client_payload.as_ref().into(),
                        ],
                        _ => unreachable!(),
                    };
                    let ingress = connection.meta().as_ref().cloned();
                    match policy
                        .send(EvalHttpFn(HttpFn::Request, args, ingress))
                        .await
                    {
                        // allow request
                        Ok(Ok((true, meta))) => {
                            // build request
                            let client_request =
                                build_request(client, connection.uri(), req, meta, timeout);
                            // forward the request (with the original client payload)
                            let res = client_request.send_body(client_payload).await;
                            // send the response back to the client
                            response(p, policy, res).await
                        }
                        // reject
                        Ok(Ok((false, _meta))) => Ok(unauthorized("bad client request")),
                        // policy error
                        Ok(Err(e)) => {
                            log::warn!("{}", e);
                            Ok(internal())
                        }
                        // actor error
                        Err(e) => {
                            log::warn!("{}", e);
                            Ok(internal())
                        }
                    }
                }
                // allow
                PolicyStatus {
                    request: FnPolicy::Allow,
                    timeout,
                    ..
                } => {
                    log::debug!("{:?}", req);
                    let mut client_payload = BytesMut::new();
                    while let Some(chunk) = payload.next().await {
                        let chunk = chunk?;
                        client_payload.extend_from_slice(&chunk)
                    }
                    // build request
                    let client_request =
                        build_request(client, connection.uri(), req, None, timeout);
                    // forward the request (with the original client payload)
                    let res = client_request.send_body(client_payload).await;
                    // send the response back to the client
                    response(p, policy, res).await
                }
                // deny
                PolicyStatus {
                    request: FnPolicy::Deny,
                    ..
                } => Ok(unauthorized("request denied")),
            }
        } else {
            // we failed to get a policy
            log::warn!("failed to get HTTP policy");
            Ok(internal())
        }
    } else {
        // could not obtain forwarding URL
        Ok(internal())
    }
}

fn response_builder(
    res: &ClientResponse<impl Stream<Item = Result<web::Bytes, PayloadError>> + Unpin>,
) -> actix_web::dev::HttpResponseBuilder {
    let mut response_builder = HttpResponse::build(res.status());
    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| {
        *h != "connection" && *h != "content-length" && *h != "content-encoding" && *h != X_ARMOUR
    }) {
        // log::debug!("header {}: {:?}", header_name, header_value);
        response_builder.header(header_name.clone(), header_value.clone());
    }
    response_builder
}

/// Send server response back to client
async fn response(
    p: HttpPolicyResponse,
    policy: web::Data<actix::Addr<PolicyActor>>,
    res: Result<
        ClientResponse<impl Stream<Item = Result<web::Bytes, PayloadError>> + Unpin>,
        SendRequestError,
    >,
) -> Result<HttpResponse, actix_web::Error> {
    match res {
        Ok(mut res) => {
            match p.status {
                // check server response
                PolicyStatus {
                    response: FnPolicy::Args(count),
                    ..
                } => {
                    let server_payload = res.body().await?;
                    let args = match count {
                        0 => vec![],
                        1 => {
                            vec![(&response_builder(&res).finish(), &p.connection).to_expression()]
                        }
                        2 => vec![
                            (&response_builder(&res).finish(), &p.connection).to_expression(),
                            server_payload.as_ref().into(),
                        ],
                        _ => unreachable!(),
                    };
                    let ingress = get_x_armour(res.headers());
                    match policy
                        .send(EvalHttpFn(HttpFn::Response, args, ingress))
                        .await
                    {
                        // allow
                        Ok(Ok((true, meta))) => {
                            let mut builder = response_builder(&res);
                            // add X-Armour header
                            if let Some(meta) = meta {
                                builder.header("x-armour", meta.as_str());
                            };
                            log::debug!("{:?}", builder);
                            Ok(builder.body(server_payload))
                        }
                        // reject
                        Ok(Ok((false, _meta))) => {
                            Ok(unauthorized("request denied (bad server response)"))
                        }
                        // policy error
                        Ok(Err(e)) => {
                            log::warn!("{}", e);
                            Ok(internal())
                        }
                        // actor error
                        Err(e) => {
                            log::warn!("{}", e);
                            Ok(internal())
                        }
                    }
                }
                // allow
                PolicyStatus {
                    response: FnPolicy::Allow,
                    ..
                } => {
                    let mut builder = response_builder(&res);
                    log::debug!("{:?}", builder);
                    Ok(builder.body(res.body().await?))
                }
                // deny
                PolicyStatus {
                    response: FnPolicy::Deny,
                    ..
                } => Ok(unauthorized("request denied (bad server response)")),
            }
        }
        // error response when connecting to server
        Err(err) => Ok(err.error_response()),
    }
}

fn internal() -> HttpResponse {
    HttpResponse::InternalServerError().body("Armour internal error")
}

fn unauthorized(message: &'static str) -> HttpResponse {
    HttpResponse::Unauthorized().body(message)
}

const X_ARMOUR: &str = "x-armour";

fn get_x_armour(h: &HeaderMap) -> Option<String> {
    h.get(X_ARMOUR)
        .map(|h| h.to_str().map(String::from).ok())
        .flatten()
}

fn build_request<U>(
    client: web::Data<Client>,
    url: U,
    req: HttpRequest,
    meta: Option<String>,
    timeout: std::time::Duration,
) -> ClientRequest
where
    uri::Uri: TryFrom<U>,
    <uri::Uri as TryFrom<U>>::Error: Into<http::Error>,
{
    // client request builder, using original request as starting point
    let mut client_req = client.request_from(url, req.head());
    // the client request headers
    let headers = client_req.headers_mut();
    // process the X-Forwarded-Host header
    let mut forward_hosts: Vec<&HeaderValue> = req.headers().get_all("x-forwarded-host").collect();
    // log::debug!("HOSTS are: {:?}", forward_hosts);
    if !forward_hosts.is_empty() {
        // we had a X-Forwarded-Host header, so need to update Host header and rebuild X-Forwarded-Host header
        headers.insert(
            HeaderName::from_static("host"),
            forward_hosts.remove(0).clone(),
        );
        headers.remove("x-forwarded-host");
        for host in forward_hosts.into_iter() {
            headers.append(HeaderName::from_static("x-forwarded-host"), host.clone())
        }
    }
    // try to add X-Forwarded-For header
    if let Some(Ok(addr)) = req
        .peer_addr()
        .map(|a| HeaderValue::from_str(a.to_string().as_str()))
    {
        headers.insert(HeaderName::from_static("x-forwarded-for"), addr)
    }
    // add X-Armour header
    if let Some(Ok(meta)) = meta.as_ref().map(|m| HeaderValue::from_str(m.as_str())) {
        headers.insert(HeaderName::from_static(X_ARMOUR), meta)
    } else {
        headers.remove(X_ARMOUR)
    }
    log::debug!("{:?}", client_req);
    client_req.timeout(timeout)
}

struct Connection {
    uri: uri::Uri,
    from: ID,
    to: ID,
    meta: Option<String>,
}

impl Connection {
    fn new(req: &HttpRequest, proxy_port: u16) -> Option<Connection> {
        // obtain the forwarding URI
        match Connection::forward_uri(req, proxy_port) {
            Ok(uri) => {
                let to = uri.clone().into();
                Some(Connection {
                    uri,
                    from: req.peer_addr().into(),
                    to,
                    meta: get_x_armour(req.headers()),
                })
            }
            Err(err) => {
                log::warn!("{}", err);
                None
            }
        }
    }
    fn uri(&self) -> &uri::Uri {
        &self.uri
    }
    fn meta(&self) -> &Option<String> {
        &self.meta
    }
    fn from_to(&self) -> (ID, ID) {
        (self.from.clone(), self.to.clone())
    }
    fn forward_uri(req: &HttpRequest, proxy_port: u16) -> Result<uri::Uri, actix_web::Error> {
        let info = req.connection_info();
        // log::debug!("HOST is: {}", info.host());
        let mut uri = uri::Builder::new()
            .scheme(info.scheme())
            .authority(info.host());
        if let Some(p_and_q) = req.uri().path_and_query() {
            uri = uri.path_and_query(p_and_q.clone());
        }
        match uri.build() {
            Ok(uri) => {
                if uri.port_u16().unwrap_or(80) == proxy_port
                    && uri.host().map(is_local_host).unwrap_or(true)
                {
                    Err(HttpResponse::InternalServerError()
                        .body("cannot proxy self")
                        .into())
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
