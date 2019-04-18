//! A simple, prototype proxy with publish-subscribe features.
//! For now, the proxy and all the clients and servers are assumed to share the same host name/IP.
//! Proxying is based on port numbers. The client is expected to embed the destination server's
//! port number within the URI path (first item).

#[macro_use]
extern crate log;

use actix_web::{middleware, server, App, HttpResponse};
use clap::{crate_version, App as ClapApp, Arg};
use std::env;
use std::net::{IpAddr, SocketAddr};

// use crate endpoint;
extern crate arm_proxy;
use arm_proxy::endpoint::parse_port;
use arm_proxy::policy;
use arm_proxy::rest_policy_utils;

/// Find a local interface's IP by name
pub fn interface_ip_addr(s: &str) -> Option<IpAddr> {
    if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
        interfaces.iter().find(|i| i.name == s).map(|i| i.ip())
    } else {
        None
    }
}

fn main() -> Result<(), std::io::Error> {
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
    // GP: Ignoring this for now
    // let mut allowed_ports = matches
    //     .values_of("allow port")
    //     .map(|ports| ports.map(|a| parse_port(a)).collect())
    //     .unwrap_or(Vec::new());
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
    // allowed_ports.sort();
    // info!("Allowed ports are: {:?}", &allowed_ports);
    let state = policy::PolicyStateL3::init();
    let st1 = state.clone_state();

    // start up the proxy server
    info!(
        "Starting proxy server: http://{}:{}",
        servername, proxy_port
    );
    let proxy_socket = SocketAddr::new(ip, proxy_port);
    server::new(move || {
        App::with_state(policy::PolicyStateL3::init_clone(st1.clone()))
            .middleware(middleware::Logger::default())
            .default_resource(|r| r.f(rest_policy_utils::forward))
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
        App::with_state(policy::PolicyStateL3::init_clone(state.clone_state()))
            .middleware(middleware::Logger::default())
            .resource("/allow/{source}/{destination}/{port}", |r| {
                r.f(rest_policy_utils::allow_host)
            })
            .default_resource(|_| HttpResponse::BadRequest())
    })
    .bind(proxy_socket)
    .expect(&format!("Failed to bind to {}", proxy_socket))
    .start();

    sys.run();

    Ok(())
}
