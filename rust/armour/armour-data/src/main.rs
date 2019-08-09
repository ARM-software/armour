use actix::prelude::*;
use armour_data::{http_proxy, policy};
use armour_data_interface::{PolicyRequest, ProxyConfig};
use clap::{crate_version, App as ClapApp, Arg};
use std::env;

fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("armour-data")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Armour Proxy, with support for Security Policies")
        .arg(
            Arg::with_name("proxy port")
                .short("p")
                .takes_value(true)
                .help("Proxy port number"),
        )
        .arg(
            Arg::with_name("master socket")
                .index(1)
                .required(true)
                .help("Unix socket of data plane master"),
        )
        .arg(
            Arg::with_name("log level")
                .short("l")
                .takes_value(true)
                .help("log level: error, warn, info, debug, trace"),
        )
        .get_matches();

    let log_level = matches.value_of("log level").unwrap_or("debug");

    // enable logging
    env::set_var(
        "RUST_LOG",
        format!("armour_data={l},actix_web={l}", l = log_level),
    );
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // process the command line arguments
    let proxy_port = matches.value_of("proxy port").map(|port| {
        port.parse()
            .unwrap_or_else(|_| panic!("bad port: {}", port))
    });

    log::info!("local host names are: {:?}", *http_proxy::LOCAL_HOST_NAMES);

    // start Actix system
    let sys = actix::System::new("armour-data");

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)

    // install the CLI policy
    let master_socket = matches.value_of("master socket").unwrap();

    let policy = policy::DataPolicy::create_policy(master_socket).unwrap_or_else(|e| {
        log::warn!(
            r#"failed to connect to data master "{}": {}"#,
            master_socket,
            e
        );
        std::process::exit(1)
    });

    // start up the proxy server
    if let Some(port) = proxy_port {
        policy.do_send(PolicyRequest::Start(ProxyConfig {
            port,
            request_streaming: false,
            response_streaming: false,
        }))
    };

    // handle Control-C
    let ctrl_c = tokio_signal::ctrl_c().flatten_stream();
    let handle_shutdown = ctrl_c
        .for_each(|()| {
            println!("Ctrl-C received, shutting down");
            System::current().stop();
            Ok(())
        })
        .map_err(|_| ());
    actix::spawn(handle_shutdown);

    sys.run()
}
