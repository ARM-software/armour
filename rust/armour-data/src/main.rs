use actix::prelude::*;
use actix_web::{client::Client, middleware, web, App, HttpServer};
use armour_data::{policy, proxy};
use clap::{crate_version, App as ClapApp, Arg};
use std::env;

fn main() -> std::io::Result<()> {
    // defaults
    const DEFAULT_PROXY_PORT: u16 = 8443;
    const DEFAULT_MASTER_SOCKET: &str = "../armour-data-master/armour";

    // CLI
    let matches = ClapApp::new("armour-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Armour Proxy, with support for Security Policies")
        .arg(
            Arg::with_name("proxy port")
                .short("p")
                .takes_value(true)
                .help(&format!(
                    "Proxy port number (default: {})",
                    DEFAULT_PROXY_PORT
                )),
        )
        .arg(
            Arg::with_name("master socket")
                .index(1)
                .required(false)
                .help("Unix socket of data plane master"),
        )
        .get_matches();

    // enable logging
    env::set_var("RUST_LOG", "armour_data=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // process the command line arguments
    let proxy_port = matches
        .value_of("proxy port")
        .map(|port| {
            port.parse()
                .unwrap_or_else(|_| panic!("bad port: {}", port))
        })
        .unwrap_or(DEFAULT_PROXY_PORT);

    // start Actix system
    let sys = actix::System::new("armour-data");

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)

    // install the CLI policy
    let master_socket = matches
        .value_of("policy file")
        .unwrap_or(DEFAULT_MASTER_SOCKET);

    let policy = policy::DataPolicy::create_policy(master_socket).unwrap_or_else(|e| {
        log::warn!(
            r#"failed to connect to data master "{}": {}"#,
            master_socket,
            e
        );
        std::process::exit(1)
    });

    // start up the proxy server
    let socket_address = format!("localhost:{}", proxy_port);
    let socket = socket_address.to_string();
    log::info!("starting proxy server: http://{}", socket);
    HttpServer::new(move || {
        App::new()
            .data(policy.clone())
            .data(Client::new())
            .data(socket.clone())
            .wrap(middleware::Logger::default())
            .default_service(web::route().to_async(proxy::proxy))
    })
    .bind(socket_address)
    .map_err(|e| {
        // stop if we cannot bind to socket address
        System::current().stop();
        e
    })?
    .start();

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
