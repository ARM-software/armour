use super::instance::InstanceSelector;
use super::master::{ArmourDataMaster, PolicyCommand};
use actix_web::{post, web, HttpResponse};
use armour_api::{labels::Label, master::PolicyUpdate, proxy::Protocol};
use armour_lang::lang::Program;
use lazy_static::lazy_static;

type Master = web::Data<actix::Addr<ArmourDataMaster>>;

lazy_static! {
	static ref MASTER_PROXY_LABEL: Label = "<master>::<proxy>".parse().unwrap();
}

#[post("/update")]
pub async fn update(
	name: web::Data<String>,
	master: Master,
	request: web::Json<PolicyUpdate>,
) -> Result<HttpResponse, actix_web::Error> {
	if let Ok(label) = request.label.parse::<Label>() {
		if let Some(m) = MASTER_PROXY_LABEL.match_with(&label) {
			let master_ok = m
				.get("master")
				.map(|master_name| master_name == **name)
				.unwrap_or(true);
			if master_ok {
				let instance_selector = m
					.get("proxy")
					.map(InstanceSelector::Name)
					.unwrap_or(InstanceSelector::All);
				match Program::from_bincode_raw(request.policy.as_bytes()) {
					Ok(prog) => {
						let prog_protocol = prog.protocol();
						if let Ok(protocol) = prog_protocol.parse::<Protocol>() {
							log::info!(
								"sending {} policy: {} {}",
								protocol,
								label,
								prog.blake3_hash().unwrap()
							);
							if let Some(err) = master
								.send(PolicyCommand(
									instance_selector,
									armour_api::proxy::PolicyRequest::SetPolicy(protocol, prog),
								))
								.await?
							{
								Ok(HttpResponse::BadRequest().body(err))
							} else {
								Ok(HttpResponse::Ok().finish())
							}
						} else {
							Ok(HttpResponse::BadRequest()
								.body(format!("unrecognized policy protocol: {}", prog_protocol)))
						}
					}
					Err(err) => Ok(HttpResponse::BadRequest()
						.body(format!("failed to parse policy bincode:\n{}", err))),
				}
			} else {
				Ok(HttpResponse::BadRequest().body("policy is not for this master"))
			}
		} else {
			Ok(HttpResponse::BadRequest().body("expecting label of the form <master>::<proxy>"))
		}
	} else {
		Ok(HttpResponse::BadRequest().body("failed to parse label"))
	}
}
