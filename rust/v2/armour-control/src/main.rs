// For MongoDB installation see: https://docs.mongodb.com/manual/tutorial/install-mongodb-on-os-x

use actix_web::{middleware, web, App, HttpServer};
use armour_control::{restapi::*, ControlPlaneState};
use listenfd::ListenFd;
use mongodb::{options::ClientOptions, Client};
#[macro_use]
extern crate clap;
use clap::{crate_version};

const DEFAULT_MONGO_DB: &str = "mongodb://localhost:27017";

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // enable logging
    std::env::set_var("RUST_LOG", "armour_control=info,actix_web=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();


    let yaml = load_yaml!("../resources/cli.yml");
    let matches = clap::App::from_yaml(yaml)
        .version(crate_version!())
        .get_matches();

    let mongo_url = matches.value_of("MONGODBURL")
        .unwrap_or(DEFAULT_MONGO_DB);

    let mut listenfd = ListenFd::from_env();

    let mut db_options = ClientOptions::parse(mongo_url).map_err(|e| {
        log::warn!("failed to get db_options");
        std::io::Error::new(std::io::ErrorKind::Other, e)
    })?;
    db_options.app_name = Some("armour".to_string());
    let client = Client::with_options(db_options.clone()).map_err(|e| {
        log::info!("Failed to connect to Mongo. Start MongoDB");
        std::io::Error::new(std::io::ErrorKind::Other, e)
    })?;
    let state = std::sync::Arc::new(ControlPlaneState {
        db_endpoint: db_options,
        db_con: client,
    });

    let mut server = HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/controlplane")
                    .service(onboard_master)
                    .service(onboard_service)
                    .service(update_policy)
                    .service(query_policy), // .service(index),
            )
    });

    server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
        if let Ok(addr) = l.local_addr() {
            log::info!("listening on: {} [via systemfd]", addr)
        }
        server.listen(l).unwrap()
    } else {
        const ADDR: &str = "127.0.0.1:8088";
        log::info!("listening on: {}", ADDR);
        server.bind(ADDR).unwrap()
    };

    server.run();
    tokio::signal::ctrl_c().await.unwrap_or_default();
    Ok(())
}
