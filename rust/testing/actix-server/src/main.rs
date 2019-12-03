use actix_web::{web, App, HttpResponse, HttpServer};
use clap::{crate_version, App as ClapApp, Arg};
use std::env;

static MESSAGE: &str = include_str!("static/nginx.html");

fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("actix-server")
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
    env::set_var("RUST_LOG", "actix_server=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // start the actix system
    let sys = actix::System::new("actix-server");

    // start up the service server
    let socket = format!("0.0.0.0:{}", port);
    HttpServer::new(|| {
        App::new()
            // .wrap(actix_web::middleware::Logger::default())
            .default_service(web::route().to(|| HttpResponse::Ok().body(MESSAGE)))
    })
    .bind(socket.clone())
    .unwrap_or_else(|_| panic!("failed to bind to http://{}", socket))
    .start();

    sys.run()
}
