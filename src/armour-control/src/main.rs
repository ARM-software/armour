// For MongoDB installation see: https://docs.mongodb.com/manual/tutorial/install-mongodb-on-os-x

use actix_web::{error, middleware, web, App, FromRequest, HttpRequest, HttpResponse, HttpServer};
use armour_control::{rest_api, ControlPlaneState};
use mongodb::{options::ClientOptions, Client};
use tokio::stream::StreamExt;

const DEFAULT_MONGO_DB: &str = "mongodb://localhost:27017";

type Error = Box<dyn std::error::Error + Send + Sync>;

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    // proceess command line arguments
    let yaml = clap::load_yaml!("../resources/cli.yml");
    let matches = clap::App::from_yaml(yaml)
        .version(clap::crate_version!())
        .get_matches();
    let mongo_url = matches.value_of("MONGODBURL").unwrap_or(DEFAULT_MONGO_DB);
    let port = matches
        .value_of("PORT")
        .map(|s| s.parse().ok())
        .flatten()
        .unwrap_or(armour_api::control::TCP_PORT);
    let control_plane = std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(0, 0, 0, 0), port);

    // enable logging
    std::env::set_var(
        "RUST_LOG",
        "armour_control=info,armour_utils=info,actix_web=info",
    );
    std::env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // connect to MongoDB
    let mut db_endpoint = ClientOptions::parse(mongo_url).await.map_err(|e| {
        log::warn!("failed to get db_endpoint");
        e
    })?;
    db_endpoint.app_name = Some("armour".to_string());
    let db_con = Client::with_options(db_endpoint.clone()).map_err(|e| {
        log::info!("Failed to connect to Mongo. Start MongoDB");
        e
    })?;

    // start from empty database
    db_con.database("armour").drop(None).await?;
    log::info!("reset armour database");
    let state = web::Data::new(ControlPlaneState {
        db_endpoint,
        db_con,
    });

    // start HTTP server
    let ca = matches
        .value_of("CA")
        .unwrap_or("certificates/armour-ca.pem");
    let certificate_password = matches.value_of("CERTIFICATE_PASSWORD").unwrap_or("armour");
    let certificate = matches
        .value_of("CERTIFICATE")
        .unwrap_or("certificates/armour-control.p12");
    let ssl_builder = armour_utils::ssl_builder(
        ca,
        certificate_password,
        certificate,
        !matches.is_present("NO_MTLS"),
    )?;
    let ca = ca.to_string();
    let certificate_password = certificate_password.to_string();
    let certificate = certificate.to_string();
    HttpServer::new(move || {
        let client = armour_utils::client(&ca, &certificate_password, &certificate)
            .expect("failed to build HTTP client");
        App::new()
            .data(client)
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/host")
                    .service(rest_api::host::list)
                    .service(rest_api::host::on_board)
                    .service(rest_api::host::drop)
                    .default_service(web::to(index)),
            )
            .service(
                web::scope("/service")
                    .service(rest_api::service::list)
                    .service(rest_api::service::on_board)
                    .service(rest_api::service::drop)
                    .default_service(web::to(index)),
            )
            .service(
                web::scope("/policy")
                    .service(rest_api::policy::list)
                    .service(rest_api::policy::update)
                    .service(rest_api::policy::update_onboarding)
                    .service(rest_api::policy::update_global)
                    .service(rest_api::policy::query)
                    .service(rest_api::policy::query_onboarding)
                    .service(rest_api::policy::query_global)
                    .service(rest_api::policy::drop)
                    .service(rest_api::policy::drop_all)
                    .service(rest_api::policy::specialize)
                    .default_service(web::to(index)),
            )
            .app_data(
                web::Json::<armour_api::control::PolicyUpdateRequest>::configure(|cfg| {
                    cfg.error_handler(json_error_handler)
                }),
            )
            .default_service(web::to(index))
    })
    .bind_openssl(control_plane, ssl_builder)?
    .run();

    log::info!("listening on: https://{}", control_plane);

    // await ^C
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
    log::warn!("{:?}\n{:?}", req, body);
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
