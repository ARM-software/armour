/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use super::connections::Connections;
use actix_files::NamedFile;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use std::sync::{Arc, Mutex};

pub fn start_web_server(connections: Arc<Mutex<Connections>>, port: u16) -> std::io::Result<()> {
    let socket_address = format!("localhost:{}", port);
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
    .run();
    log::info!("starting web server: http://{}", socket_address);
    Ok(())
}

async fn service_graph(
    connections: web::Data<Arc<Mutex<Connections>>>,
) -> std::io::Result<NamedFile> {
    connections
        .lock()
        .unwrap()
        .export_svg("connections_service", true, true)?;
    Ok(NamedFile::open("connections_service.svg")?)
}

async fn graph(connections: web::Data<Arc<Mutex<Connections>>>) -> std::io::Result<NamedFile> {
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
