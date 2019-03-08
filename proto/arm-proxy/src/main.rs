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
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::{env, fmt};
use url::Url;

// Shared pub/sub state:
// - For now, topics are simple strings (to be replaced with a hierarchy of topics)
// - Also there's no database for storing/retrieving messages

#[derive(Clone)]
struct Subscribers(HashMap<String, HashSet<u16>>);

impl Subscribers {
    fn new() -> Subscribers {
        Subscribers(HashMap::new())
    }
    fn add_subscriber(&mut self, port: u16, topic: &str) {
        if let Some(set) = self.0.get_mut(topic) {
            set.insert(port);
        } else {
            let mut set = HashSet::new();
            set.insert(port);
            self.0.insert(topic.to_string(), set);
        }
    }
}

struct PubSubState {
    subscribers: Arc<Mutex<Subscribers>>,
}

impl<'a> PubSubState {
    pub fn init(subscribers: Arc<Mutex<Subscribers>>) -> PubSubState {
        PubSubState { subscribers }
    }
    fn subscribers(&self, topic: &str) -> HashSet<u16> {
        if let Some(set) = (*self.subscribers.lock().unwrap()).0.get(topic) {
            set.clone()
        } else {
            HashSet::new()
        }
    }
}

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
    PathQuery,
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
        ForwardUrlError::PathQuery
    }
}

fn forward_url(uri: &str) -> Result<Url, ForwardUrlError> {
    let url = Url::parse(uri)?;
    match (url.host(), url.path_segments(), url.query()) {
        (Some(host), Some(mut path_segs), query) => {
            // the port is the first item of the path
            if let Some(port) = path_segs.next() {
                // copy over the rest of the path
                let mut path = String::new();
                for p in path_segs {
                    path.push_str("/");
                    path.push_str(p)
                }
                // create the server URL (same host)
                let mut new_url = Url::parse(&format!("http://{}", host))?;
                new_url.set_port(Some(port.parse()?))?;
                new_url.set_path(&path);
                new_url.set_query(query);
                Ok(new_url)
            } else {
                Err(ForwardUrlError::PathQuery)
            }
        }
        _ => Err(ForwardUrlError::PathQuery),
    }
}

fn is_allowed(state: &ProxyState, port: u16) -> bool {
    debug!("allowed ports are {}", *state);
    state.allow.lock().unwrap().contains(&port)
}

/// Forward request from client sender to a destination server
fn forward(req: &HttpRequest<ProxyState>) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let info = req.connection_info();
    let full_uri = format!("{}://{}{}", info.scheme(), info.host(), req.uri());
    match forward_url(&full_uri) {
        Ok(server_url) => {
            let server_port = server_url.port().unwrap();
            if is_allowed(req.state(), server_port) {
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
            } else {
                Box::new(fut_ok(
                    HttpResponse::Forbidden()
                        .body(&format!("access to server {} is blocked", server_port)),
                ))
            }
        }
        Err(err) => Box::new(fut_ok(HttpResponse::BadRequest().body(&format!(
            "failed to construct server URL from {} {:?}",
            full_uri, err
        )))),
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

/// Subscribe to topic
fn subscribe(req: &HttpRequest<PubSubState>) -> HttpResponse {
    if let Ok(params) = Path::<(u16, String)>::extract(req) {
        (*req.state().subscribers.lock().unwrap()).add_subscriber(params.0, &params.1);
        let s = format!("added subscriber {} to topic \"{}\"", params.0, params.1);
        info!("{}", s);
        HttpResponse::Ok().body(s)
    } else {
        HttpResponse::BadRequest().finish()
    }
}

/// Publish to topic
fn publish(req: HttpRequest<PubSubState>) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    if let Ok(topic) = Path::<String>::extract(&req) {
        let subscribers = req.state().subscribers(&*topic);
        if subscribers.len() == 0 {
            Box::new(fut_ok(HttpResponse::BadRequest().body(&format!(
                "there are no subscribers to topic \"{}\"",
                *topic
            ))))
        } else {
            let info = req.connection_info();
            let url = Url::parse(&format!(
                "{}://{}/subscription/{}",
                info.scheme(),
                info.host(),
                *topic
            ))
            .unwrap();
            Box::new(req.body().from_err().and_then(move |body| {
                for subscriber_port in subscribers.iter() {
                    let mut subscriber_url = url.clone();
                    subscriber_url.set_port(Some(*subscriber_port)).unwrap();
                    info!("sending \"{}\" topic to {}", *topic, subscriber_url);
                    actix::spawn(
                        client::ClientRequest::put(subscriber_url.clone())
                            .body(body.clone())
                            .unwrap()
                            .send()
                            .and_then(|_| Ok(()))
                            // should unsubscribe on error?
                            .map_err(move |_| error!("got error publishing to {}", subscriber_url)),
                    )
                }
                fut_ok(HttpResponse::Ok().body(format!("published to topic \"{}\"", *topic)))
            }))
        }
    } else {
        Box::new(fut_ok(HttpResponse::BadRequest().finish()))
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

fn main() {
    // defaults
    let default_proxy_port = 8443;
    let default_interface = "en0";

    // CLI
    let matches = ClapApp::new("arm-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Proxy with support for Security Policies")
        .arg(
            Arg::with_name("proxy port")
                .required(false)
                .index(1)
                .help(&format!(
                    "Proxy port number (default: {})",
                    default_proxy_port
                )),
        )
        .arg(
            Arg::with_name("pub/sub port")
                .short("p")
                .takes_value(true)
                .help("Pub/sub port number (off if absent)"),
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
        .map(|port| port.parse().expect(&format!("bad port: {}", port)))
        .unwrap_or(8443);
    let pubsub_port = matches
        .value_of("pub/sub port")
        .map(|port| Some(port.parse().expect(&format!("bad port: {}", port))))
        .unwrap_or(None);
    let mut allowed_ports = matches
        .values_of("allow port")
        .map(|ports| {
            ports
                .map(|a| a.parse().expect(&format!("bad port: {}", a)))
                .collect()
        })
        .unwrap_or(Vec::new());
    assert_ne!(Some(proxy_port), pubsub_port);
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

    // start up the proxy server
    allowed_ports.sort();
    info!("Allowed ports are: {:?}", &allowed_ports);
    let proxy_state = Arc::new(Mutex::new(HashSet::from_iter(allowed_ports)));

    let proxy_socket = SocketAddr::new(ip, proxy_port);
    let proxy_server = server::new(move || {
        App::with_state(ProxyState::init(proxy_state.clone()))
            .middleware(middleware::Logger::default())
            .resource("/allow/{port}", |r| r.f(allow))
            .default_resource(|r| r.f(forward))
    })
    .bind(proxy_socket)
    .expect(&format!("Failed to bind to {}", proxy_socket));
    info!(
        "Starting proxy server: http://{}:{}",
        servername, proxy_port
    );
    proxy_server.start();

    // perhaps start pub/sub server
    if let Some(port) = pubsub_port {
        let pubsub_state = Arc::new(Mutex::new(Subscribers::new()));
        let pubsub_socket = SocketAddr::new(ip, port);
        let pubsub_server = server::new(move || {
            App::with_state(PubSubState::init(pubsub_state.clone()))
                .middleware(middleware::Logger::default())
                .resource("/subscribe/{port}/{topic}", |r| r.f(subscribe))
                .resource("/publish/{topic}", |r| r.with(publish))
                .default_resource(|_r| HttpResponse::BadRequest())
        })
        .bind(pubsub_socket)
        .expect(&format!("Failed to bind to {}", proxy_socket));
        info!("Starting pub/sub broker: http://{}:{}", servername, port);
        pubsub_server.start();
    }

    sys.run();
}
