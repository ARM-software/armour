use armour_data::proxy;
use clap::{crate_version, App, Arg};
use std::env;

fn main() -> Result<(), std::io::Error> {
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
    env::set_var("RUST_LOG", "armour_proxy=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // start the actix system
    let sys = actix::System::new("armour-proxy");

    // shared state
    let state = ();

    // start up the proxy server
    proxy::start(state, format!("localhost:{}", proxy_port));

    sys.run();

    Ok(())
}
