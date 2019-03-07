//! A simple REST client

#[macro_use]
extern crate log;

use actix_web::{
    actix, client, http::Method, middleware, server, App, Error, FromRequest, HttpMessage,
    HttpRequest, HttpResponse, Path,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::{future::ok as fut_ok, Future};
use std::env;
use std::net::{IpAddr, SocketAddr};

// shared pub/sub state (should be replaced with proper database)
struct ServiceState {
    port: u16,
}

impl<'a> ServiceState {
    pub fn init(port: u16) -> ServiceState {
        ServiceState { port }
    }
}

/// Respond to requests
fn service(req: HttpRequest<ServiceState>) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    Box::new(req.body().from_err().and_then(move |body| {
        fut_ok(HttpResponse::Ok().body(format!(
            "port {} received request {} with body {:?} from {}",
            req.state().port,
            req.uri(),
            body,
            req.connection_info().host(),
        )))
    }))
}

/// Respond to publish notification
fn subscription(
    req: HttpRequest<ServiceState>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    if let Ok(topic) = Path::<String>::extract(&req) {
        Box::new(req.body().from_err().and_then(move |body| {
            info!("got message: {:?} on topic \"{}\"", body, *topic);
            fut_ok(HttpResponse::Ok().finish())
        }))
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
    let default_pubsub_port = 8444;
    let default_interface = "en0";

    // CLI
    let matches = ClapApp::new("arm-service")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Proxy with support for Security Policies")
        .arg(
            Arg::with_name("own port")
                .required(false)
                .short("o")
                .takes_value(true)
                .help("own port"),
        )
        .arg(
            Arg::with_name("proxy port")
                .required(false)
                .short("p")
                .takes_value(true)
                .help(&format!(
                    "Proxy port number (default: {})",
                    default_proxy_port
                )),
        )
        .arg(
            Arg::with_name("pubsub port")
                .required(false)
                .short("s")
                .takes_value(true)
                .help(&format!(
                    "Pub/sub port number (default: {})",
                    default_pubsub_port
                )),
        )
        .arg(
            Arg::with_name("server port")
                .required(false)
                .short("d")
                .takes_value(true)
                .help("desination port"),
        )
        .arg(
            Arg::with_name("route")
                .required(false)
                .short("r")
                .takes_value(true)
                .help("route path"),
        )
        .arg(
            Arg::with_name("message")
                .required(false)
                .index(1)
                .help("message"),
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

    let own_port = matches
        .value_of("own port")
        .map(|l| l.parse().expect(&format!("bad port: {}", l)));
    let proxy_port = matches
        .value_of("proxy port")
        .map(|l| l.parse().expect(&format!("bad port: {}", l)))
        .unwrap_or(default_proxy_port);
    let pubsub_port = matches
        .value_of("pubsub port")
        .map(|l| l.parse().expect(&format!("bad port: {}", l)))
        .unwrap_or(default_pubsub_port);
    let server_port: Option<u16> = matches
        .value_of("server port")
        .map(|l| l.parse().expect(&format!("bad port: {}", l)));
    let route = matches.value_of("route").unwrap_or("");
    let message = matches
        .value_of("message")
        .unwrap_or("<no message>")
        .to_string();
    let interface = matches.value_of("interface").unwrap_or(default_interface);

    // get the server name and IP address of the named interface
    let ip = interface_ip_addr(interface).expect("Failed to obtain IP address");
    let servername = hostname::get_hostname().unwrap_or(ip.to_string());

    // enable logging
    env::set_var("RUST_LOG", "arm_service=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // start the actix system
    let sys = actix::System::new("arm-proxy");

    // start up the service server
    if let Some(port) = own_port {
        let socket = SocketAddr::new(ip, port);
        let server = server::new(move || {
            App::with_state(ServiceState::init(port))
                .middleware(middleware::Logger::default())
                .route("/subscription/{topic}", Method::PUT, subscription)
                .default_resource(|r| r.with(service))
        })
        .bind(socket)
        .expect(&format!("Failed to bind to {}", socket));
        info!("Starting service: http://{}:{}", servername, port);
        server.start();
    }

    // send a message
    if let Some(destination_port) = server_port {
        actix::spawn({
            let uri = if destination_port == proxy_port || destination_port == pubsub_port {
                format!("http://{}:{}/{}", servername, destination_port, route)
            } else {
                format!(
                    "http://{}:{}/{}/{}",
                    servername, proxy_port, destination_port, route
                )
            };
            info!("sending: {}", uri);
            client::get(uri)
                .header("User-Agent", "Actix-web")
                .body(message)
                .unwrap()
                .send()
                .map_err(Error::from)
                .and_then(move |resp| {
                    resp.body().from_err().and_then(move |body| {
                        if own_port.is_none() {
                            actix::System::current().stop()
                        };
                        Ok(
                            if let Ok(text) = String::from_utf8(body.as_ref().to_vec()) {
                                println!("{:?}: {}", resp.status(), text)
                            } else {
                                println!("{:?}: {:?}", resp.status(), body)
                            },
                        )
                    })
                })
                .map_err(|_| ())
        })
    }

    sys.run();
}
