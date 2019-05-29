//! A simple stub HTTP proxy

#[macro_use]
extern crate log;

use actix_web::{
    client, middleware, server, App, AsyncResponder, Error, HttpMessage, HttpRequest, HttpResponse,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::{future, Future};
use std::env;

fn main() -> Result<(), std::io::Error> {
    // CLI
    let servername = "localhost";
    let default_proxy_port: u16 = 8443;

    let proxy_port = ClapApp::new("arm-stub-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Stub Proxy")
        // .setting(clap::AppSettings::AllowLeadingHyphen)
        .arg(
            Arg::with_name("proxy port")
                .short("p")
                .takes_value(true)
                .help(&format!(
                    "proxy port number (default: {})",
                    default_proxy_port
                )),
        )
        .get_matches()
        .value_of("proxy port")
        .map(|port| port.parse().expect(&format!("bad port: {}", port)))
        .unwrap_or(default_proxy_port);

    // enable logging
    env::set_var("RUST_LOG", "arm_stub_proxy=debug,actix_web=debug");
    env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // start the actix system
    let sys = actix::System::new("arm-stub-proxy");

    // start up the proxy server
    let proxy_socket = format!("{}:{}", servername, proxy_port);
    info!("Starting proxy server: http://{}", proxy_socket);
    server::new(move || {
        App::new()
            .middleware(middleware::Logger::default())
            .default_resource(|r| r.f(forward))
    })
    .bind(proxy_socket.clone())
    .expect(&format!("Failed to bind to {}", proxy_socket))
    .start();

    sys.run();

    Ok(())
}

/// Forward request from client sender to a destination server
fn forward(req: &HttpRequest) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let info = req.connection_info();
    match url::Url::parse(&format!("{}://{}{}", info.scheme(), info.host(), req.uri())) {
        Ok(server_url) => client::ClientRequest::build_from(req)
            .no_default_headers()
            .uri(server_url)
            .streaming(req.payload())
            .unwrap()
            .set_x_forward_for(req.peer_addr())
            .send()
            .map_err(Error::from)
            .and_then(construct_response)
            .responder(),
        Err(err) => Box::new(future::ok(
            HttpResponse::BadRequest().body(&format!("failed to construct server URL {:?}", err)),
        )),
    }
}

trait SetForwardFor {
    fn set_x_forward_for(self, a: Option<std::net::SocketAddr>) -> Self;
}

impl SetForwardFor for client::ClientRequest {
    fn set_x_forward_for(mut self, a: Option<std::net::SocketAddr>) -> Self {
        if let Some(addr) = a {
            match self.headers_mut().entry("x-forwarded-for") {
                Ok(http::header::Entry::Vacant(entry)) => {
                    let addr = format!("{}", addr.ip());
                    entry.insert(addr.parse().unwrap());
                }
                Ok(http::header::Entry::Occupied(mut entry)) => {
                    let addr = format!("{}, {}", entry.get().to_str().unwrap(), addr.ip());
                    entry.insert(addr.parse().unwrap());
                }
                _ => unreachable!(),
            }
            self
        } else {
            self
        }
    }
}

/// Forward response from detination server back to client sender
fn construct_response(
    resp: client::ClientResponse,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let mut client_resp = HttpResponse::build(resp.status());
    for (header_name, header_value) in resp.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.header(header_name.clone(), header_value.clone());
    }
    if resp.chunked().unwrap_or(false) {
        Box::new(future::ok(client_resp.streaming(resp.payload())))
    } else {
        Box::new(
            resp.body()
                .from_err()
                .and_then(move |body| Ok(client_resp.body(body))),
        )
    }
}
