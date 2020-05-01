// For MongoDB installation see: https://docs.mongodb.com/manual/tutorial/install-mongodb-on-os-x

use actix_web::{error, middleware, web, App, FromRequest, HttpRequest, HttpResponse, HttpServer};
use armour_control::{rest_api, ControlPlaneState};
use listenfd::ListenFd;
use mongodb::{options::ClientOptions, Client};
use tokio::stream::StreamExt;

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
    // start from blank database
    db_con.database("armour").drop(None)?;
    log::info!("reset armour database");
    let state = web::Data::new(ControlPlaneState {
        db_endpoint,
        db_con,
    });

    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/master")
                    .service(rest_api::master::on_board)
                    .service(rest_api::master::drop)
                    .default_service(web::to(index)),
            )
            .service(
                web::scope("/service")
                    .service(rest_api::service::on_board)
                    .service(rest_api::service::drop)
                    .default_service(web::to(index)),
            )
            .service(
                web::scope("/policy")
                    .service(rest_api::policy::update)
                    .service(rest_api::policy::query)
                    .service(rest_api::policy::drop)
                    .service(rest_api::policy::drop_all)
                    .default_service(web::to(index)),
            )
            .app_data(
                web::Json::<armour_api::control::PolicyUpdateRequest>::configure(|cfg| {
                    cfg.error_handler(json_error_handler)
                }),
            )
            .default_service(web::to(index))
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

async fn index(
    req: HttpRequest,
    mut payload: actix_web::web::Payload,
) -> Result<HttpResponse, actix_web::Error> {
    let mut body = bytes::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk)
    }
    log::warn!("hello: {:?}\n{:?}", req, body);
    Ok(actix_web::HttpResponse::BadRequest().finish())
}

fn json_error_handler(err: error::JsonPayloadError, _req: &HttpRequest) -> error::Error {
    use actix_web::error::JsonPayloadError;

    let detail = err.to_string();
    let resp = match &err {
        JsonPayloadError::ContentType => HttpResponse::UnsupportedMediaType().body(detail),
        JsonPayloadError::Deserialize(json_err) if json_err.is_data() => {
            HttpResponse::UnprocessableEntity().body(detail)
        }
        _ => HttpResponse::BadRequest().body(detail),
    };
    error::InternalError::from_response(err, resp).into()
}
