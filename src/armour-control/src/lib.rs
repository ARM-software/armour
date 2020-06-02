use mongodb::{options::ClientOptions, Client};

pub struct ControlPlaneState {
	pub db_endpoint: ClientOptions,
	pub db_con: Client,
}

// pub mod data_model;
pub mod rest_api;