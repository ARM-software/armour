use armour_lang::lang::Program;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OnboardMasterRequest {
	pub host: Url,
	pub credentials: String, // FIXME change types as needed
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceRequest {
	pub label: String,  // FIXME
	pub master: String, // FIXME
}

#[derive(Serialize, Deserialize)]
pub struct PolicyUpdateRequest {
	pub service: String, // FIXME
	pub policy: Program,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQuery {
	pub service: String, // FIXME
}
