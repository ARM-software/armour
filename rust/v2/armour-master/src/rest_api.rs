type Master = actix_web::web::Data<actix::Addr<super::master::ArmourDataMaster>>;

use actix_web::{post, web, HttpResponse};

#[post("/on-board")]
pub async fn onboard(
	info: web::Json<armour_api::master::OnboardInformation>,
) -> Result<HttpResponse, actix_web::Error> {
	// TODO: onboarding
	log::info!(
		"received onboarding: {}",
		serde_json::to_string_pretty(&info.0)?
	);
	Ok(HttpResponse::Ok().finish())
}

pub mod master {
	use crate::master::List;
	use actix_web::{get, web, HttpResponse};

	#[get("/name")]
	pub async fn name(name: web::Data<String>) -> HttpResponse {
		HttpResponse::Ok().body(name.to_string())
	}
	#[get("/proxies")]
	pub async fn proxies(master: super::Master) -> HttpResponse {
		match master.send(List).await {
			Ok(res) => HttpResponse::Ok().json2(&*res),
			Err(err) => {
				log::warn!("{}", err);
				HttpResponse::InternalServerError().finish()
			}
		}
	}
}

pub mod policy {
	use crate::instance::InstanceSelector;
	use crate::master::{MetaData, PolicyCommand};
	use actix_web::{get, post, web, HttpResponse};
	use armour_api::{
		master::PolicyUpdate,
		proxy::{Policy, PolicyRequest},
	};
	use armour_lang::labels::Label;
	use lazy_static::lazy_static;
	use std::convert::TryFrom;

	lazy_static! {
		static ref MASTER_PROXY_LABEL: Label = "<master>::<proxy>".parse().unwrap();
	}

	fn match_label(label: &str, name: &str) -> Result<InstanceSelector, &'static str> {
		if let Ok(label) = label.parse::<Label>() {
			if let Some(m) = MASTER_PROXY_LABEL.match_with(&label) {
				if m.get("master")
					.map(|master_name| master_name == name)
					.unwrap_or(true)
				{
					Ok(m.get("proxy")
						.map(InstanceSelector::Name)
						.unwrap_or(InstanceSelector::All))
				} else {
					Err("label not for this master")
				}
			} else {
				Err("expecting label of the form <master>::<proxy>")
			}
		} else {
			Err("failed to parse label")
		}
	}

	#[get("/query")]
	pub async fn query(
		name: web::Data<String>,
		master: super::Master,
		request: web::Json<String>,
	) -> HttpResponse {
		match match_label(&request, &name) {
			Ok(instance) => match master.send(MetaData(instance)).await {
				Ok(res) => HttpResponse::Ok().json2(&*res),
				Err(err) => {
					log::warn!("{}", err);
					HttpResponse::InternalServerError().finish()
				}
			},
			Err(err) => HttpResponse::BadRequest().body(err),
		}
	}

	#[post("/update")]
	pub async fn update(
		name: web::Data<String>,
		master: super::Master,
		request: web::Json<PolicyUpdate>,
	) -> HttpResponse {
		match match_label(&request.label, &name) {
			Ok(instance) => match Policy::try_from(&request.policy) {
				Ok(policy) => {
					log::info!("sending policy: {}", policy);
					match master
						.send(PolicyCommand(instance, PolicyRequest::SetPolicy(policy)))
						.await
					{
						Ok(None) => HttpResponse::Ok().finish(),
						Ok(Some(err)) => HttpResponse::BadRequest().body(err),
						Err(err) => {
							log::warn!("{}", err);
							HttpResponse::InternalServerError().finish()
						}
					}
				}
				Err(err) => HttpResponse::BadRequest().body(err),
			},
			Err(err) => HttpResponse::BadRequest().body(err),
		}
	}
}
