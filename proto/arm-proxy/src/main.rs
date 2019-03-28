//! A simple, prototype proxy with publish-subscribe features.
//! For now, the proxy and all the clients and servers are assumed to share the same host name/IP.
//! Proxying is based on port numbers. The client is expected to embed the destination server's
//! port number within the URI path (first item).

#[macro_use]
extern crate log;

use actix_web::{
    client, middleware, server, App, AsyncResponder, Error, FromRequest, HttpMessage, HttpRequest,
    HttpResponse, Path,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::{future::ok as fut_ok, Future};
use std::collections::HashSet;
use std::iter::FromIterator;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::{env, fmt};
use url::Url;

// shared state that whitelists traffic to a destination port (to be replaced by full policy check)
// use of mutex could be an issue for efficiency/scaling!
type Policy = Arc<Mutex<HashSet<u16>>>;
pub struct ProxyState {
    pub allow: Policy,
}

impl<'a> ProxyState {
    pub fn init(allow: Policy) -> ProxyState {
        ProxyState { allow }
    }
    fn port_allowed(&self, port: &u16) -> bool {
        debug!("allowed ports are {}", self);
        self.allow.lock().unwrap().contains(&port)
    }
}

impl fmt::Display for ProxyState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut vec = Vec::from_iter((*self.allow.lock().unwrap()).clone());
        vec.sort();
        writeln!(f, "{:?}", vec)
    }
}

#[derive(Debug)]
enum ForwardUrlError {
    ParseError(url::ParseError),
    ParseIntError(std::num::ParseIntError),
    Blocked(u16),
    ForwardPort,
}

impl From<url::ParseError> for ForwardUrlError {
    fn from(err: url::ParseError) -> ForwardUrlError {
        ForwardUrlError::ParseError(err)
    }
}

impl From<std::num::ParseIntError> for ForwardUrlError {
    fn from(err: std::num::ParseIntError) -> ForwardUrlError {
        ForwardUrlError::ParseIntError(err)
    }
}

impl From<()> for ForwardUrlError {
    fn from(_err: ()) -> ForwardUrlError {
        ForwardUrlError::ForwardPort
    }
}

fn get_forward_to_port(req: &HttpRequest<ProxyState>) -> Option<u16> {
    if let Some(p) = req.headers().get("forward-to-port") {
        if let Ok(s) = p.to_str() {
            s.parse().ok()
        } else {
            None
        }
    } else {
        None
    }
}

fn forward_url(req: &HttpRequest<ProxyState>) -> Result<Url, ForwardUrlError> {
    if let Some(forward_port) = get_forward_to_port(req) {
        if req.state().port_allowed(&forward_port) {
            let info = req.connection_info();
            let mut url = Url::parse(&format!("{}://{}{}", info.scheme(), info.host(), req.uri()))?;
            url.set_port(Some(forward_port))?;
            Ok(url)
        } else {
            Err(ForwardUrlError::Blocked(forward_port))
        }
    } else {
        Err(ForwardUrlError::ForwardPort)
    }
}

/// Forward request from client sender to a destination server
fn forward(req: &HttpRequest<ProxyState>) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    match forward_url(req) {
        Ok(server_url) => {
            let mut forwarded_req = client::ClientRequest::build_from(req)
                .no_default_headers()
                .uri(server_url)
                .streaming(req.payload())
                .unwrap();

            if let Some(addr) = req.peer_addr() {
                match forwarded_req.headers_mut().entry("x-forwarded-for") {
                    Ok(http::header::Entry::Vacant(entry)) => {
                        let addr = format!("{}", addr.ip());
                        entry.insert(addr.parse().unwrap());
                    }
                    Ok(http::header::Entry::Occupied(mut entry)) => {
                        let addr = format!("{}, {}", entry.get().to_str().unwrap(), addr.ip());
                        entry.insert(addr.parse().unwrap());
                    }
                    _ => unreachable!(),
                }
            }

            forwarded_req
                .send()
                .map_err(Error::from)
                .and_then(construct_response)
                .responder()
        }
        Err(ForwardUrlError::Blocked(port)) => Box::new(fut_ok(
            HttpResponse::Forbidden().body(&format!("access to server {} is blocked", port)),
        )),
        Err(err) => Box::new(fut_ok(
            HttpResponse::BadRequest().body(&format!("failed to construct server URL {:?}", err)),
        )),
    }
}

/// Forward response from detination server back to client sender
fn construct_response(
    resp: client::ClientResponse,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let mut client_resp = HttpResponse::build(resp.status());
    for (header_name, header_value) in resp.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.header(header_name.clone(), header_value.clone());
    }
    if resp.chunked().unwrap_or(false) {
        Box::new(fut_ok(client_resp.streaming(resp.payload())))
    } else {
        Box::new(
            resp.body()
                .from_err()
                .and_then(move |body| Ok(client_resp.body(body))),
        )
    }
}

/// Forward response from detination server back to client sender
fn allow(req: &HttpRequest<ProxyState>) -> String {
    if let Ok(port) = Path::<u16>::extract(req) {
        (*req.state().allow.lock().unwrap()).insert(*port);
        let s = format!("Added port {}", *port);
        debug!("{}, allowed ports are {}", s, req.state());
        s
    } else {
        "".to_string()
    }
}

/// Find a local interface's IP by name
fn interface_ip_addr(s: &str) -> Option<IpAddr> {
    if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
        interfaces.iter().find(|i| i.name == s).map(|i| i.ip())
    } else {
        None
    }
}

fn parse_port(s: &str) -> u16 {
    s.parse().expect(&format!("bad port: {}", s))
}

fn main() {
    // defaults
    let default_proxy_port = 8443;
    let default_proxy_control_port = 8444;
    let default_interface = "en0";

    // CLI
    let matches = ClapApp::new("arm-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Proxy with support for Security Policies")
        .arg(
            Arg::with_name("proxy port")
                .short("p")
                .takes_value(true)
                .help(&format!(
                    "Proxy port number (default: {})",
                    default_proxy_port
                )),
        )
        .arg(
            Arg::with_name("proxy control port")
                .short("o")
                .takes_value(true)
                .help(&format!(
                    "Proxy control port number (default: {})",
                    default_proxy_control_port
                )),
        )
        .arg(
            Arg::with_name("interface")
                .short("i")
                .long("interface")
                .takes_value(true)
                .help(&format!(
                    "name of interface (default: {})",
                    default_interface
                )),
        )
        .arg(
            Arg::with_name("allow port")
                .short("a")
                .multiple(true)
                .number_of_values(1)
                .help("Allow forwarding to port number"),
        )
        .get_matches();

    // process the commmand line arguments
    let proxy_port = matches
        .value_of("proxy port")
        .map(|port| parse_port(port))
        .unwrap_or(default_proxy_port);
    let proxy_control_port = matches
        .value_of("proxy control port")
        .map(|port| parse_port(port))
        .unwrap_or(default_proxy_control_port);
    let mut allowed_ports = matches
        .values_of("allow port")
        .map(|ports| ports.map(|a| parse_port(a)).collect())
        .unwrap_or(Vec::new());
    let interface = matches.value_of("interface").unwrap_or(default_interface);

    // get the server name and the IP address for the named interface (e.g. "en0" or "lo")
    let ip = interface_ip_addr(interface).expect("Failed to obtain IP address");
    let servername = hostname::get_hostname().unwrap_or(ip.to_string());

    // enable logging
    env::set_var("RUST_LOG", "arm_proxy=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // start the actix system
    let sys = actix::System::new("arm-proxy");

    // shared state
    allowed_ports.sort();
    info!("Allowed ports are: {:?}", &allowed_ports);
    let proxy_allow_state = Arc::new(Mutex::new(HashSet::from_iter(allowed_ports)));
    let clone_proxy_state = proxy_allow_state.clone();

    // start up the proxy server
    info!(
        "Starting proxy server: http://{}:{}",
        servername, proxy_port
    );
    let proxy_socket = SocketAddr::new(ip, proxy_port);
    server::new(move || {
        App::with_state(ProxyState::init(proxy_allow_state.clone()))
            .middleware(middleware::Logger::default())
            .default_resource(|r| r.f(forward))
    })
    .bind(proxy_socket)
    .expect(&format!("Failed to bind to {}", proxy_socket))
    .start();

    // start up the proxy control server
    info!(
        "Starting proxy controller: http://{}:{}",
        servername, proxy_control_port
    );
    let proxy_socket = SocketAddr::new(ip, proxy_control_port);
    server::new(move || {
        App::with_state(ProxyState::init(clone_proxy_state.clone()))
            .middleware(middleware::Logger::default())
            .resource("/allow/{port}", |r| r.f(allow))
            .default_resource(|_| HttpResponse::BadRequest())
    })
    .bind(proxy_socket)
    .expect(&format!("Failed to bind to {}", proxy_socket))
    .start();

    sys.run();
}
