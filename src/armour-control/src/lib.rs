use mongodb::{options::ClientOptions, Client};
use actix_web::web;

pub struct ControlPlaneState {
	pub db_endpoint: ClientOptions,
	pub db_con: Client,
}

pub type State = web::Data<ControlPlaneState>;

// pub mod data_model;
pub mod interpret;
pub mod policy;
pub mod rest_api;
pub mod specialize;
