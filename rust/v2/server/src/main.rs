use actix_web::{ web, post, App, HttpResponse, HttpServer};
use std::collections::BTreeMap as Map;

#[post("/on-boarding")]
async fn index(item: web::Json<Map<String, armour_compose::service::MasterInfo>>) -> HttpResponse {
    println!("model: {:?}", &item);
    //todo in master: store/process labels before sending
    HttpResponse::Ok().json(item.0)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(index)
    })
    .bind("127.0.0.1:8088")?
    .run()
    .await
}