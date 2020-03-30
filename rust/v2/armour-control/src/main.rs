// For MongoDB installation see: https://docs.mongodb.com/manual/tutorial/install-mongodb-on-os-x

use actix_web::{middleware, web, App, HttpServer};
use armour_control::{restapi::*, ControlPlaneState};
use listenfd::ListenFd;
use mongodb::{options::ClientOptions, Client};

const DEFAULT_MONGO_DB: &str = "mongodb://localhost:27017";

type Error = Box<dyn std::error::Error + Send + Sync>;

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    // enable logging
    std::env::set_var("RUST_LOG", "armour_control=info,actix_web=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    let yaml = clap::load_yaml!("../resources/cli.yml");
    let matches = clap::App::from_yaml(yaml)
        .version(clap::crate_version!())
        .get_matches();

    let mongo_url = matches.value_of("MONGODBURL").unwrap_or(DEFAULT_MONGO_DB);

    let mut listenfd = ListenFd::from_env();

    let mut db_endpoint = ClientOptions::parse(mongo_url).map_err(|e| {
        log::warn!("failed to get db_endpoint");
        e
    })?;
    db_endpoint.app_name = Some("armour".to_string());
    let db_con = Client::with_options(db_endpoint.clone()).map_err(|e| {
        log::info!("Failed to connect to Mongo. Start MongoDB");
        e
    })?;
    let state = web::Data::new(ControlPlaneState {
        db_endpoint,
        db_con,
    });

    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
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
