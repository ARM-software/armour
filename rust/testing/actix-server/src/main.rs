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
        .arg(
            Arg::with_name("workers")
                .required(false)
                .short("w")
                .takes_value(true)
                .help("number of workers"),
        )
        .arg(
            Arg::with_name("backlog")
                .required(false)
                .short("b")
                .takes_value(true)
                .next_line_help(true)
                .help("maximum number of pending connections\n(default is 2048)"),
        )
        .arg(
            Arg::with_name("maxconn")
                .required(false)
                .short("m")
                .takes_value(true)
                .next_line_help(true)
                .help("maximum per-worker number of concurrent connections\n(default is 25k)"),
        )
        .arg(
            Arg::with_name("maxconnrate")
                .required(false)
                .short("r")
                .takes_value(true)
                .next_line_help(true)
                .help(
                    "maximum per-worker concurrent connection establish process\n(default is 256)",
                ),
        )
        .get_matches();

    // start up the service server
    let port = matches
        .value_of("port")
        .map(|p| p.parse().unwrap_or(80))
        .unwrap_or(80);
    let socket = format!("0.0.0.0:{}", port);
    println!("starting server at {}", socket);
    let mut server = HttpServer::new(|| {
        App::new()
            // .wrap(actix_web::middleware::Logger::default())
            .default_service(web::route().to(|| HttpResponse::Ok().body(MESSAGE)))
    })
    .bind(socket.clone())
    .unwrap_or_else(|_| panic!("failed to bind to http://{}", socket));

    if let Some(Ok(w)) = matches.value_of("workers").map(|w| w.parse::<usize>()) {
        server = server.workers(w);
    }
    if let Some(Ok(b)) = matches.value_of("backlog").map(|w| w.parse::<i32>()) {
        server = server.backlog(b);
    }
    if let Some(Ok(m)) = matches.value_of("maxconn").map(|w| w.parse::<usize>()) {
        server = server.maxconn(m);
    }
    if let Some(Ok(m)) = matches.value_of("maxconnrate").map(|w| w.parse::<usize>()) {
        server = server.maxconnrate(m);
    }

    server.run()
}
