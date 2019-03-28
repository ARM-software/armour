//! A simple REST client

#[macro_use]
extern crate log;

use actix_web::{
    actix, client, http::Method, middleware, server, App, Error, FromRequest, HttpMessage,
    HttpRequest, HttpResponse, Path,
};
use clap::{crate_version, App as ClapApp, AppSettings, Arg};
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

fn parse_port(s: &str) -> u16 {
    s.parse().expect(&format!("bad port: {}", s))
}

fn main() {
    // defaults
    let default_interface = "en0";

    // CLI
    let matches = ClapApp::new("arm-service")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Proxy with support for Security Policies")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("own port")
                .required(false)
                .short("o")
                .takes_value(true)
                .help("own port"),
        )
        .arg(
            Arg::with_name("destination port")
                .required(false)
                .short("d")
                .takes_value(true)
                .help("desination port"),
        )
        .arg(
            Arg::with_name("forward port")
                .required(false)
                .short("f")
                .takes_value(true)
                .help("forward to port"),
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

    let own_port = matches.value_of("own port").map(|l| parse_port(l));
    let destination_port = matches.value_of("destination port").map(|l| parse_port(l));
    let forward_port = matches.value_of("forward port").map(|l| parse_port(l));
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
    if let Some(destination_port) = destination_port {
        actix::spawn({
            let uri = format!("http://{}:{}/{}", servername, destination_port, route);
            info!("sending: {}", uri);
            let mut builder = client::get(uri);
            if let Some(forward_port) = forward_port {
                builder.header("forward-to-port", forward_port.to_string());
            }
            builder
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
