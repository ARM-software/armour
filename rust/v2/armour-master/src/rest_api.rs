type Master = actix_web::web::Data<actix::Addr<super::master::ArmourDataMaster>>;

pub mod launch {
	use actix_web::{client, http::Method};
	use actix_web::{delete, post, web, HttpResponse};
	use armour_api::control::OnboardServiceRequest;
	use armour_api::master::OnboardInformation;
	use armour_lang::labels::Label;
	use armour_serde::array_dict::ArrayDict;
	use std::collections::BTreeMap;

	fn get_armour_id(labels: &ArrayDict) -> Result<Label, actix_web::Error> {
		static ERR: &str = "failed to get Armour ID for service";
		match labels {
			ArrayDict::Array(a) => {
				if a.len() == 1 {
					Ok(a[0]
						.parse()
						.map_err(|_| HttpResponse::BadRequest().body(ERR))?)
				} else {
					Err(HttpResponse::BadRequest().body(ERR).into())
				}
			}
			ArrayDict::Dict(d) => {
				if let Some(v) = d.get("id") {
					Ok(v.parse()
						.map_err(|_| HttpResponse::BadRequest().body(ERR))?)
				} else {
					Err(HttpResponse::BadRequest().body(ERR).into())
				}
			}
		}
	}

	fn onboard_requests(
		master: &Label,
		info: OnboardInformation,
	) -> Result<BTreeMap<String, OnboardServiceRequest>, actix_web::Error> {
		info.into_iter()
			.map(|(k, v)| {
				Ok((
					k,
					OnboardServiceRequest {
						service: get_armour_id(&v.armour_labels)?,
						master: master.to_owned(),
					},
				))
			})
			.collect()
	}

	#[post("/on-board-services")]
	pub async fn on_board_services(
		master: web::Data<Label>,
		info: web::Json<OnboardInformation>,
	) -> Result<HttpResponse, actix_web::Error> {
		let client = client::Client::default();
		for (service, req) in onboard_requests(&master, info.into_inner())? {
			crate::control_plane(&client, Method::POST, "on-board-service", &req)
				.await
				.map_err(|message| {
					HttpResponse::BadRequest().body(format!(
						"on-boarding failed for service {}: {}",
						service, message
					))
				})?;
			log::info!("onboarded {}", service)
		}
		Ok(HttpResponse::Ok().finish())
	}

	#[delete("/drop-services")]
	pub async fn drop_services(
		master: web::Data<Label>,
		info: web::Json<OnboardInformation>,
	) -> Result<HttpResponse, actix_web::Error> {
		let client = client::Client::default();
		for (service, req) in onboard_requests(&master, info.into_inner())? {
			crate::control_plane(&client, Method::DELETE, "drop-service", &req)
				.await
				.map_err(|message| {
					HttpResponse::BadRequest()
						.body(format!("drop failed for service {}: {}", service, message))
				})?;
			log::info!("dropped {}", service)
		}
		Ok(HttpResponse::Ok().finish())
	}
}

pub mod master {
	use crate::master::List;
	use actix_web::{get, web, HttpResponse};

	#[get("/label")]
	pub async fn label(label: web::Data<armour_lang::labels::Label>) -> HttpResponse {
		HttpResponse::Ok().body(label.to_string())
	}
	#[get("/proxies")]
	pub async fn proxies(master: super::Master) -> Result<HttpResponse, actix_web::Error> {
		let res = master.send(List).await.map_err(|err| {
			log::warn!("{}", err);
			HttpResponse::InternalServerError()
		})?;
		Ok(HttpResponse::Ok().json2(&*res))
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
	use std::convert::TryFrom;

	fn instance_selector(label: &str, proxy: &Label) -> Result<InstanceSelector, &'static str> {
		let label = label
			.parse::<Label>()
			.map_err(|_| "failed to parse label")?;
		let (first, rest) = label.split_first().ok_or("bad label")?;
		if first.matches_with(proxy) {
			Ok(if let Some(proxy) = rest {
				InstanceSelector::Label(proxy)
			} else {
				InstanceSelector::All
			})
		} else {
			Err("label not for this master")
		}
	}

	#[get("/query")]
	pub async fn query(
		label: web::Data<Label>,
		master: super::Master,
		request: web::Json<String>,
	) -> Result<HttpResponse, actix_web::Error> {
		let instance = instance_selector(&request, &label)
			.map_err(|err| HttpResponse::BadRequest().body(err))?;
		let res = master.send(MetaData(instance)).await.map_err(|err| {
			log::warn!("{}", err);
			HttpResponse::InternalServerError()
		})?;
		Ok(HttpResponse::Ok().json2(&*res))
	}

	#[post("/update")]
	pub async fn update(
		label: web::Data<Label>,
		master: super::Master,
		request: web::Json<PolicyUpdate>,
	) -> Result<HttpResponse, actix_web::Error> {
		let instance = instance_selector(&request.label, &label)
			.map_err(|err| HttpResponse::BadRequest().body(err))?;
		let policy = Policy::try_from(&request.policy)
			.map_err(|err| HttpResponse::BadRequest().body(err))?;
		log::info!("sending policy: {}", policy);
		let res = master
			.send(PolicyCommand(instance, PolicyRequest::SetPolicy(policy)))
			.await
			.map_err(|err| {
				log::warn!("{}", err);
				HttpResponse::InternalServerError()
			})?;
		match res {
			None => Ok(HttpResponse::Ok().finish()),
			Some(err) => Ok(HttpResponse::BadRequest().body(err)),
		}
	}
}
