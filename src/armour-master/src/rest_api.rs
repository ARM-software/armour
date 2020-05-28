type Master = actix::Addr<super::master::ArmourDataMaster>;

pub mod service {
	use crate::instance::InstanceSelector;
	use crate::master::{Launch, PolicyCommand};
	use actix_web::{delete, post, web, HttpResponse};
	use armour_api::master::{OnboardInformation, Proxies, Proxy};
	use armour_api::proxy::{LabelOp, PolicyRequest};
	use armour_lang::labels::Labels;

	async fn launch_proxy(master: &super::Master, proxy: &Proxy) -> Result<(), actix_web::Error> {
		// start a proxy (without forcing/duplication)
		master
			.send(Launch::new(
				proxy.label.clone(),
				false,
				if proxy.debug {
					log::Level::Debug
				} else {
					log::Level::Warn
				},
				proxy.timeout,
			))
			.await?;
		Ok(())
	}

	async fn add_ip_labels(
		master: &super::Master,
		instance: &InstanceSelector,
		ip_labels: &[(std::net::Ipv4Addr, Labels)],
	) -> Result<(), actix_web::Error> {
		master
			.send(PolicyCommand::new_with_retry(
				// retry needed in case proxy process is slow to start up
				instance.clone(),
				PolicyRequest::Label(LabelOp::AddIp(ip_labels.to_vec())),
			))
			.await?;
		Ok(())
	}

	async fn start_proxy(
		master: &super::Master,
		instance: InstanceSelector,
		port: u16,
	) -> Result<(), actix_web::Error> {
		master
			.send(PolicyCommand::new_with_retry(
				// retry needed in case proxy process is slow to start up
				instance,
				PolicyRequest::StartHttp(port),
			))
			.await?;
		Ok(())
	}

	#[post("/on-board")]
	pub async fn on_board(
		master: web::Data<super::Master>,
		information: web::Json<OnboardInformation>,
	) -> Result<HttpResponse, actix_web::Error> {
		let information = information.into_inner();
		let mut port = information.top_port();
		for proxy in information.proxies {
			// launch proxies (if not already launched)
			launch_proxy(&master, &proxy).await?;
			let instance = InstanceSelector::Label(proxy.label.clone());
			// add service labels
			add_ip_labels(&master, &instance, &information.labels).await?;
			start_proxy(
				&master,
				instance,
				proxy.port.unwrap_or_else(|| {
					port += 1;
					port
				}),
			)
			.await?
		}
		log::info!("onboarded");
		Ok(HttpResponse::Ok().finish())
	}

	#[delete("/drop")]
	pub async fn drop(
		master: web::Data<super::Master>,
		proxies: web::Json<Proxies>,
	) -> Result<HttpResponse, actix_web::Error> {
		for proxy in proxies.into_inner() {
			let instance = InstanceSelector::Label(proxy.label);
			// shut down proxy
			master
				.send(PolicyCommand::new(instance, PolicyRequest::Shutdown))
				.await?;
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
	pub async fn proxies(
		master: web::Data<super::Master>,
	) -> Result<HttpResponse, actix_web::Error> {
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
	use armour_api::{master::PolicyUpdate, proxy::PolicyRequest};
	use armour_lang::labels::Label;

	#[get("/query")]
	pub async fn query(
		master: web::Data<super::Master>,
		request: web::Json<Label>,
	) -> Result<HttpResponse, actix_web::Error> {
		let instance = InstanceSelector::Label(request.clone());
		let res = master.send(MetaData(instance)).await.map_err(|err| {
			log::warn!("{}", err);
			HttpResponse::InternalServerError()
		})?;
		Ok(HttpResponse::Ok().json2(&*res))
	}

	#[post("/update")]
	pub async fn update(
		master: web::Data<super::Master>,
		request: web::Json<PolicyUpdate>,
	) -> Result<HttpResponse, actix_web::Error> {
		let instance = InstanceSelector::Label(request.label.clone());
		log::info!("sending policy: {}", request.policy);
		let res = master
			.send(PolicyCommand::new(
				instance,
				PolicyRequest::SetPolicy(request.policy.clone()),
			))
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
