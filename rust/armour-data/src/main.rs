use armour_data::{policy, proxy};
use clap::{crate_version, App, Arg};
use std::env;

fn main() -> std::io::Result<()> {
    // defaults
    let default_proxy_port: u16 = 8443;

    // CLI
    let matches = App::new("armour-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Armour Proxy, with support for Security Policies")
        .arg(
            Arg::with_name("proxy port")
                .short("p")
                .takes_value(true)
                .help(&format!(
                    "Proxy port number (default: {})",
                    default_proxy_port
                )),
        )
        .get_matches();

    // process the commmand line arguments
    let proxy_port = matches
        .value_of("proxy port")
        .map(|port| port.parse().expect(&format!("bad port: {}", port)))
        .unwrap_or(default_proxy_port);

    // enable logging
    env::set_var("RUST_LOG", "armour_data=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // shared state
    let mut policy = policy::ArmourPolicy::new();
    policy.from_file("test2.policy").unwrap_or(());

    // start up the proxy server
    proxy::start(policy, format!("localhost:{}", proxy_port))
}
