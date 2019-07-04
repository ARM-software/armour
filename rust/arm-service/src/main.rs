//! A simple REST client

#[macro_use]
extern crate log;

use actix_web::{client, middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use clap::{crate_version, App as ClapApp, AppSettings, Arg};
use futures::stream::Stream;
use futures::{future, lazy, Future};
use std::env;


fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("arm-service")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Proxy with support for Security Policies")
        .setting(AppSettings::ArgRequiredElseHelp)
        .arg(
            Arg::with_name("own port")
                .required(false)
                .short("o")
                .takes_value(true)
                .help("own port"),
        )
        .arg(
            Arg::with_name("proxy")
                .required(false)
                .short("p")
                .takes_value(true)
                .help("proxy port or socket"),
        )
        .arg(
            Arg::with_name("destination")
                .required(false)
                .short("d")
                .takes_value(true)
                .help("desination port or socket"),
        )
        .arg(
            Arg::with_name("uri")
                .required(false)
                .short("u")
                .takes_value(true)
                .help("URI"),
        )
        .arg(
            Arg::with_name("message")
                .required(false)
                .index(1)
                .help("message"),
        )
        .get_matches();

    let own_port = matches.value_of("own port").map(|l| parse_port(l));
    let proxy = matches.value_of("proxy").map(str::to_string);
    let destination = matches.value_of("destination").map(str::to_string);
    let uri = matches.value_of("uri").unwrap_or("");
    let message = matches
        .value_of("message")
        .unwrap_or("<no message>")
        .to_string();

    // enable logging
    env::set_var("RUST_LOG", "arm_service=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // start the actix system
    let sys = actix::System::new("arm-service");

    // start up the service server
    if let Some(port) = own_port {
        let socket = format!("localhost:{}", port);
        let server = HttpServer::new(move || {
            App::new()
                .data(port)
                .wrap(middleware::Logger::default())
                .default_service(web::route().to_async(service))
        })
        .bind(socket.clone())
        .expect(&format!("failed to bind to http://{}", socket));
        info!("starting service: {}", socket);
        server.start();
    }

    // send a message
    if let Some(destination) = destination {
        let uri = format!(
            "http://{}/{}",
            host(&proxy.clone().unwrap_or(destination.clone())),
            uri
        );
        info!("sending: {}", uri);
        let mut client = client::Client::new().get(uri);
        if proxy.is_some() {
            client = client.header("X-Forwarded-Host", host(&destination))
        };
        actix::Arbiter::spawn(lazy(move || {
            client
                .send_body(message)
                .map_err(|_| ())
                .and_then(move |mut resp| {
                    resp.body().map_err(|_| ()).and_then(move |body| {
                        if own_port.is_none() {
                            actix::System::current().stop()
                        };
                        future::ok(
                            if let Ok(text) = String::from_utf8(body.as_ref().to_vec()) {
                                println!("{:?}: {}", resp.status(), text)
                            } else {
                                println!("{:?}: {:?}", resp.status(), body)
                            },
                        )
                    })
                })
        }));
    };

    sys.run()
}

fn parse_port(s: &str) -> u16 {
    s.parse().expect(&format!("bad port: {}", s))
}

fn host(s: &str) -> String {
    if let Ok(port) = s.parse::<u16>() {
        format!("localhost:{}", port)
    } else {
        s.to_string()
    }
}

/// Respond to requests
fn service(
    req: HttpRequest,
    body: web::Payload,
    port: web::Data<u16>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    body.map_err(Error::from)
        .fold(web::BytesMut::new(), |mut body, chunk| {
            body.extend_from_slice(&chunk);
            Ok::<_, Error>(body)
        })
        .and_then(move |data| {
            // Ok(HttpResponse::NotFound().body("not here!"))
            // Ok(HttpResponse::Ok().body("hello"))
            let info = req.connection_info();
            Ok(HttpResponse::Ok().body(format!(
                r#"port {} received request {} with body {:?}; host {}; remote {}"#,
                port.get_ref(),
                req.uri(),
                data,
                info.host(),
                info.remote().unwrap_or("<unknown>"),
                // req.headers()
                //     .get("x-forwarded-for")
                //     .map(|v| v.to_str().unwrap_or("not a string"))
                //     .unwrap_or("not set"),
            )))
        })
}
