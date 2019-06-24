use actix::prelude::*;
use actix_web::{client::Client, middleware, web, App, HttpServer};
use armour_data::{policy, proxy};
use armour_data_interface::ArmourPolicyRequest;
use clap::{crate_version, App as ClapApp, Arg};
use std::env;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    // defaults
    let default_proxy_port: u16 = 8443;

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
                    default_proxy_port
                )),
        )
        .arg(
            Arg::with_name("policy file")
                .index(1)
                .required(false)
                .help("Policy file"),
        )
        .get_matches();

    // enable logging
    env::set_var("RUST_LOG", "armour_data=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // process the commmand line arguments
    let proxy_port = matches
        .value_of("proxy port")
        .map(|port| port.parse().expect(&format!("bad port: {}", port)))
        .unwrap_or(default_proxy_port);

    // start Actix system
    let sys = actix::System::new("armour-data");

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)
    let policy_addr = policy::ArmourPolicy::create_policy("armour");

    // install the CLI policy
    if let Some(file) = matches.value_of("policy file") {
        policy_addr.do_send(ArmourPolicyRequest::UpdateFromFile(PathBuf::from(file)))
    }

    let policy_addr_clone = policy_addr.clone();
    std::thread::spawn(move || loop {
        let mut cmd = String::new();
        if std::io::stdin().read_line(&mut cmd).is_err() {
            println!("error");
            return;
        }
        policy_addr_clone.do_send(ArmourPolicyRequest::UpdateFromFile(PathBuf::from(
            cmd.trim_end_matches('\n'),
        )));
    });

    // start up the proxy server
    let socket_address = format!("localhost:{}", proxy_port);
    let socket = socket_address.to_string();
    log::info!("starting proxy server: http://{}", socket);
    HttpServer::new(move || {
        App::new()
            .data(policy_addr.clone())
            .data(Client::new())
            .data(socket.clone())
            .wrap(middleware::Logger::default())
            .default_service(web::route().to_async(proxy::proxy))
    })
    .bind(socket_address)?
    .system_exit()
    .start();

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
