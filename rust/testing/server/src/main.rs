use actix_web::{web, App, HttpResponse, HttpServer};
use clap::{crate_version, App as ClapApp, Arg};
use std::env;

fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("server")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Fake Nginx Server")
        .arg(
            Arg::with_name("port")
                .required(false)
                .short("p")
                .takes_value(true)
                .help("port"),
        )
        .get_matches();

    let port = matches
        .value_of("port")
        .map(|p| p.parse().unwrap_or(80))
        .unwrap_or(80);

    // enable logging
    env::set_var("RUST_LOG", "server=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // start the actix system
    let sys = actix::System::new("server");

    let message = include_str!("static/nginx.html");

    // start up the service server
    let socket = format!("0.0.0.0:{}", port);
    let server = HttpServer::new(move || {
        App::new()
            .data(port)
            // .wrap(actix_web::middleware::Logger::default())
            .default_service(web::route().to(move || HttpResponse::Ok().body(message)))
    })
    .bind(socket.clone())
    .unwrap_or_else(|_| panic!("failed to bind to http://{}", socket));
    log::info!("starting service: {}", socket);
    server.start();

    sys.run()
}
