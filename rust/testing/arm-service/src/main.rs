//! A simple REST server and client

#[macro_use]
extern crate log;

// use actix_web::middleware;
use actix_web::{client, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use bytes::BytesMut;
use clap::{crate_version, App as ClapApp, AppSettings, Arg};
use futures::StreamExt;
use std::env;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
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
                .multiple(true)
                .number_of_values(1)
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
            Arg::with_name("host")
                .required(false)
                .short("h")
                .takes_value(true)
                .help("host header"),
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
    let mut proxies: Vec<&str> = matches.values_of("proxy").unwrap_or_default().collect();
    let destination = matches.value_of("destination").map(str::to_string);
    let uri = matches.value_of("uri").unwrap_or("");
    let message = matches.value_of("message").unwrap_or("").trim().to_string();
    // let message = actix_web::web::Bytes::from_static(&[1u8; 10_485_760]);

    // enable logging
    env::set_var("RUST_LOG", "arm_service=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // send a message
    if let Some(destination) = destination {
        let (dst, has_proxy) = if !proxies.is_empty() {
            (proxies.remove(0).to_string(), true)
        } else {
            (destination.clone(), false)
        };
        let uri = format!("http://{}/{}", host(&dst), uri);
        info!("sending: {}", uri);
        let mut client = client::Client::new().get(uri);
        for proxy in proxies {
            // possible intermediate proxies
            client = client.header("X-Forwarded-Host", host(proxy))
        }
        if has_proxy {
            // the final destination
            client = client.header("X-Forwarded-Host", host(&destination));
        };
        if let Some(host) = matches.value_of("host") {
            client = client.header("host", host)
        };
        // let bytes = include_bytes!("");
        match client.send_body(message).await {
            Ok(mut resp) => {
                let mut data = BytesMut::new();
                while let Some(chunk) = resp.next().await {
                    let chunk = chunk.map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                    })?;
                    data.extend_from_slice(&chunk)
                }
                if let Ok(text) = String::from_utf8(data.as_ref().to_vec()) {
                    info!("{}", text)
                } else {
                    info!("{:?}", data)
                }
            }
            Err(e) => warn!("{}", e),
        }
    }
    // start up the service server
    if let Some(port) = own_port {
        let socket = format!("0.0.0.0:{}", port);
        HttpServer::new(move || {
            App::new()
                .data(port)
                // .wrap(middleware::Logger::default())
                .default_service(web::route().to(service))
        })
        .bind(socket.clone())
        .unwrap_or_else(|_| panic!("failed to bind to http://{}", socket))
        .run();
        info!("started listening on: {}", socket);
        tokio::signal::ctrl_c().await.unwrap();
        println!("Ctrl-C received, shutting down")
    }

    Ok(())
}

fn parse_port(s: &str) -> u16 {
    s.parse().unwrap_or_else(|_| panic!("bad port: {}", s))
}

fn host(s: &str) -> String {
    if let Ok(port) = s.parse::<u16>() {
        format!("localhost:{}", port)
    } else {
        s.to_string()
    }
}

/// Respond to requests
async fn service(
    req: HttpRequest,
    mut payload: web::Payload,
    port: web::Data<u16>,
) -> Result<HttpResponse, Error> {
    let mut data = BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        data.extend_from_slice(&chunk)
    }
    // Ok(HttpResponse::NotFound().body("not here!"))
    // Ok(HttpResponse::Ok().body("hello"))
    debug!("{:?}", req);
    let info = req.connection_info();
    Ok(HttpResponse::Ok().body(format!(
        r#"port {} received request {} with body {:?}; host {}; remote {}"#,
        port.get_ref(),
        req.uri(),
        if data.len() < 4096 {
            data
        } else {
            BytesMut::from(format!("<{} bytes>", data.len()).as_str())
        },
        info.host(),
        info.remote().unwrap_or("<unknown>"),
    )))
}
