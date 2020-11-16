type Host = actix::Addr<super::host::ArmourDataHost>;

pub mod service {
	use crate::host::{Launch, PolicyCommand};
	use crate::instance::InstanceSelector;
	use actix_web::{delete, post, web, HttpResponse};
	use armour_api::host::{OnboardInformation, Proxies, Proxy};
	use armour_api::proxy::{HttpConfig, LabelOp, PolicyRequest};
	use armour_lang::labels::{Label, Labels};
	use std::str::FromStr;

	#[derive(Debug)]
	struct MailboxError;

	impl std::fmt::Display for MailboxError {
		fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
			write!(f, "mailbox error")
		}
	}
	impl From<actix::MailboxError> for MailboxError {
		fn from(_: actix::MailboxError) -> Self {
			MailboxError
		}
	}
	impl actix_web::ResponseError for MailboxError {
		fn error_response(&self) -> HttpResponse {
			log::warn!("mailbox error");
			HttpResponse::BadRequest().finish()
		}
	}

	async fn launch_proxy(host: &super::Host, proxy: &Proxy, ip_labels: Vec<(std::net::Ipv4Addr, Labels)>) -> Result<(), actix_web::Error> {
		// start a proxy (without forcing/duplication)
		host.send(Launch::new(
			proxy.label.clone(),
			false,
			if proxy.debug {
				log::Level::Debug
			} else {
				log::Level::Info
			},
			proxy.timeout,
			ip_labels
		))
		.await?;
		Ok(())
	}

	async fn add_ip_labels(
		host: &super::Host,
		instance: &InstanceSelector,
		ip_labels: &[(std::net::Ipv4Addr, Labels)],
	) -> Result<(), MailboxError> {
		host.send(PolicyCommand::new_with_retry(
			// retry needed in case proxy process is slow to start up
			instance.clone(),
			PolicyRequest::Label(LabelOp::AddIp(ip_labels.to_vec())),
		))
		.await?;
		Ok(())
	}

	async fn start_proxy(
		host: &super::Host,
		instance: InstanceSelector,
		config: HttpConfig,
	) -> Result<(), MailboxError> {
		host.send(PolicyCommand::new_with_retry(
			// retry needed in case proxy process is slow to start up
			instance,
			PolicyRequest::StartHttp(config),
		))
		.await?;
		Ok(())
	}

	#[post("/on-board")]
	pub async fn on_board(
		host: web::Data<super::Host>,
		information: web::Json<OnboardInformation>,
	) -> Result<HttpResponse, actix_web::Error> {
		let information = information.into_inner();
		let port = information.top_port();			

		for mut proxy in information.proxies {
			//FIXME: add labels ProxyType::EgressIngress and not change main proxy labels
			proxy.label = Label::concat(&Label::from_str("EgressIngress").unwrap(), &proxy.label);

			// launch proxies (if not already launched)
			launch_proxy(&host, &proxy, information.labels.clone()).await?;
			let instance = InstanceSelector::Label(proxy.label.clone());
			
			// add service labels
			add_ip_labels(&host, &instance, &information.labels).await?;
			let config = proxy.config(port);
			start_proxy(&host, instance, config).await?
		}
		log::info!("onboarded");
		Ok(HttpResponse::Ok().finish())
	}

	#[delete("/drop")]
	pub async fn drop(
		host: web::Data<super::Host>,
		proxies: web::Json<Proxies>,
	) -> Result<HttpResponse, actix_web::Error> {
		for proxy in proxies.into_inner() {
			let instance = InstanceSelector::Label(proxy.label);
			// shut down proxy
			host.send(PolicyCommand::new(instance, PolicyRequest::Shutdown))
				.await
				.map_err(|_| MailboxError)?;
		}
		Ok(HttpResponse::Ok().finish())
	}
}

pub mod host {
	use crate::host::List;
	use actix_web::{get, web, HttpResponse};

	#[get("/label")]
	pub async fn label(label: web::Data<armour_lang::labels::Label>) -> HttpResponse {
		HttpResponse::Ok().body(label.to_string())
	}
	#[get("/proxies")]
	pub async fn proxies(host: web::Data<super::Host>) -> Result<HttpResponse, actix_web::Error> {
		let res = host.send(List).await.map_err(|err| {
			log::warn!("{}", err);
			HttpResponse::InternalServerError()
		})?;
		Ok(HttpResponse::Ok().json2(&*res))
	}
}

pub mod policy {
	use crate::host::{MetaData, PolicyCommand};
	use crate::instance::InstanceSelector;
	use actix_web::{get, post, web, HttpResponse};
	use armour_api::{host::PolicyUpdate, proxy::PolicyRequest};
	use armour_lang::labels::Label;

	#[get("/query")]
	pub async fn query(
		host: web::Data<super::Host>,
		request: web::Json<Label>,
	) -> Result<HttpResponse, actix_web::Error> {
		let instance = InstanceSelector::Label(request.clone());
		let res = host.send(MetaData(instance)).await.map_err(|err| {
			log::warn!("{}", err);
			HttpResponse::InternalServerError()
		})?;
		Ok(HttpResponse::Ok().json2(&*res))
	}

	#[post("/update")]
	pub async fn update(
		host: web::Data<super::Host>,
		request: web::Json<PolicyUpdate>,
	) -> Result<HttpResponse, actix_web::Error> {
		let instance = InstanceSelector::Label(request.label.clone());
		log::info!("sending policy: {}", request.policy);
		let res = host
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
