// extern crate log;
extern crate log;

extern crate env_logger;

use actix_web::{web, App, HttpServer};
use listenfd::ListenFd;

use mongodb::{options::ClientOptions, Client};

use ctrl_plane::restapi::*;
use ctrl_plane::ControlPlaneState;

fn main() {
	env_logger::init();

	let mut listenfd = ListenFd::from_env();

	let mut server = HttpServer::new(|| {
		App::new()
			.data({
				let mut db_options =
					ClientOptions::parse("mongodb://localhost:27017").unwrap();
				db_options.app_name = Some("armour".to_string());
				let client = Client::with_options(db_options.clone()).unwrap();
				let state = ControlPlaneState {
					db_endpoint: db_options,
					db_con: client,
				};
				state
			})
			.service(
				web::scope("/controlplane")
					.service(onboard_master)
					.service(onboard_service)
					.service(update_policy)
					.service(query_policy)
					// .service(index),
			)
	});

	server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
		server.listen(l).unwrap()
	} else {
		server.bind("127.0.0.1:8088").unwrap()
	};

	server.run().unwrap();
}
