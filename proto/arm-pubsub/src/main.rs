//! A simple REST based Pub/Sub service
#[macro_use]
extern crate log;

use actix_web::{
    client, middleware, server, App, Error, FromRequest, HttpMessage, HttpRequest, HttpResponse,
    Path,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::{future::ok as fut_ok, Future};
use std::collections::{HashMap, HashSet};
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
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
    let default_pubsub_port = 8444;
    let default_interface = "en0";

    // CLI
    let matches = ClapApp::new("arm-pubsub")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Basic REST pub/sub service")
        .arg(
            Arg::with_name("pub/sub port")
                .required(false)
                .index(1)
                .help(&format!(
                    "Pub/Sub port number (default: {})",
                    default_pubsub_port
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
        .get_matches();

    // process the commmand line arguments
    let pubsub_port = matches
        .value_of("pub/sub port")
        .map(|port| port.parse().expect(&format!("bad port: {}", port)))
        .unwrap_or(default_pubsub_port);
    let interface = matches.value_of("interface").unwrap_or(default_interface);

    // get the server name and the IP address for the named interface (e.g. "en0" or "lo")
    let ip = interface_ip_addr(interface).expect("Failed to obtain IP address");
    let servername = hostname::get_hostname().unwrap_or(ip.to_string());

    // enable logging
    env::set_var("RUST_LOG", "arm_pubsub=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // start the actix system
    let sys = actix::System::new("arm-pubsub");

    let pubsub_state = Arc::new(Mutex::new(Subscribers::new()));
    let pubsub_socket = SocketAddr::new(ip, pubsub_port);
    let pubsub_server = server::new(move || {
        App::with_state(PubSubState::init(pubsub_state.clone()))
            .middleware(middleware::Logger::default())
            .resource("/subscribe/{port}/{topic}", |r| r.f(subscribe))
            .resource("/publish/{topic}", |r| r.with(publish))
            .default_resource(|_r| HttpResponse::BadRequest())
    })
    .bind(pubsub_socket)
    .expect(&format!("Failed to bind to {}", pubsub_socket));
    info!(
        "Starting pub/sub broker: http://{}:{}",
        servername, pubsub_port
    );
    pubsub_server.start();

    sys.run();
}
