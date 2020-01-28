use armour_api::proxy::PolicyRequest;
use armour_proxy::{http_proxy, policy::PolicyActor};
use clap::{crate_version, App as ClapApp, Arg};
use std::env;

fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("armour-proxy")
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
        format!(
            "armour_proxy={l},armour_lang={l},actix_web={l}",
            l = log_level
        ),
    );
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // start Actix system
    let mut sys = actix_rt::System::new("armour_proxy");

    // process the command line arguments
    let proxy_port = matches.value_of("proxy port").map(|port| {
        port.parse::<u16>()
            .unwrap_or_else(|_| panic!("bad port: {}", port))
    });

    log::info!("local host names are: {:?}", *http_proxy::LOCAL_HOST_NAMES);

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)
    // install the CLI policy
    let master_socket = matches.value_of("master socket").unwrap().to_string();
    log::info!("connecting to: {}", master_socket);
    let stream = sys.block_on(tokio::net::UnixStream::connect(master_socket))?;
    let policy = PolicyActor::create_policy(stream);

    // start a proxy server
    if let Some(port) = proxy_port {
        policy.do_send(PolicyRequest::StartHttp(port))
    };

    sys.run()
}
