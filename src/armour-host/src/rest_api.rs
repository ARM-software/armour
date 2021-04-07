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

type Host = actix::Addr<super::host::ArmourDataHost>;

pub mod service {
	use crate::host::{Launch, PolicyCommand};
	use crate::instance::InstanceSelector;
	use actix_web::{delete, post, web, HttpResponse};
	use armour_api::host::{OnboardInformation, Proxies, Proxy};
	use armour_api::proxy::{HttpConfig, LabelOp, PolicyRequest};
	use armour_lang::labels::{Labels};
	use std::collections::{HashMap};
	
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

	async fn launch_proxy(host: &super::Host, proxy: &Proxy) -> Result<(), MailboxError> {
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

	async fn start_onboarding(
		host: &super::Host,
		instance: InstanceSelector,
		ip_labels: &[(std::net::Ipv4Addr, Labels)],
	) -> Result<(), MailboxError> {
		let mut ip_labels_h : HashMap<std::net::IpAddr, Labels>= HashMap::new();

		for (ip, labels) in ip_labels {
			if let Some(labels2) = ip_labels_h.get_mut(&std::net::IpAddr::from(ip.clone())) {
				labels2.extend(labels.clone());
			} else {
				ip_labels_h.insert(std::net::IpAddr::from(ip.clone()), labels.clone());
			}
		}

		host.send(PolicyCommand::new_with_retry(
			// retry needed in case proxy process is slow to start up
			instance,
			PolicyRequest::CPOnboard(
				ip_labels_h
			),
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
		for proxy in information.proxies {
			// launch proxies (if not already launched)
			launch_proxy(&host, &proxy).await?;
			let instance = InstanceSelector::Label(proxy.label.clone());
			// add service labels
			add_ip_labels(&host, &instance, &information.labels).await?;
			let config = proxy.config(port);
			start_onboarding(&host, instance.clone(), &information.labels).await?;
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

