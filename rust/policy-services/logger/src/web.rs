use super::connections::Connections;
use actix_files::NamedFile;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use std::sync::{Arc, Mutex};

pub fn start_web_server(connections: Arc<Mutex<Connections>>, port: u16) -> std::io::Result<()> {
    let socket_address = format!("127.0.0.1:{}", port);
    let _server = HttpServer::new(move || {
        App::new()
            .data(connections.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource("/connections").to(table))
            .service(web::resource("/").to(table))
            .service(web::resource("/service_graph").to(service_graph))
            .service(web::resource("/graph").to(graph))
            .default_service(web::route().to(|| HttpResponse::NotFound().body("nothing here")))
    })
    .bind(&socket_address)?
    .start();
    log::info!("starting web server: http://{}", socket_address);
    Ok(())
}

fn service_graph(connections: web::Data<Arc<Mutex<Connections>>>) -> std::io::Result<NamedFile> {
    connections
        .lock()
        .unwrap()
        .export_svg("connections_service", true, true)?;
    Ok(NamedFile::open("connections_service.svg")?)
}

fn graph(connections: web::Data<Arc<Mutex<Connections>>>) -> std::io::Result<NamedFile> {
    connections
        .lock()
        .unwrap()
        .export_svg("connections", false, true)?;
    Ok(NamedFile::open("connections.svg")?)
}

fn table(connections: web::Data<Arc<Mutex<Connections>>>) -> HttpResponse {
    HttpResponse::Ok().body(page(
        "connections",
        &connections.lock().unwrap().html_table(),
    ))
}

fn page(title: &str, body: &str) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
<head>
<title>{}</title>
</head>
<body>
{}
</body>
</html>"#,
        title, body
    )
}
