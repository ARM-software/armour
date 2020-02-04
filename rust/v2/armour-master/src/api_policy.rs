use super::instance::InstanceSelector;
use super::master::{ArmourDataMaster, PolicyCommand};
use actix_web::{post, web, HttpResponse};
use armour_api::{
	labels::Label,
	master::{Policy, PolicyUpdate},
	proxy::{PolicyRequest, Protocol},
};
use armour_lang::lang::{Program, HTTP_POLICY, TCP_POLICY};
use lazy_static::lazy_static;
use std::io;

type Master = web::Data<actix::Addr<ArmourDataMaster>>;

lazy_static! {
	static ref MASTER_PROXY_LABEL: Label = "<master>::<proxy>".parse().unwrap();
}

fn policy(p: &Policy) -> std::io::Result<armour_lang::lang::Program> {
	match p {
		Policy::AllowAll(Protocol::TCP) => Program::allow_all(&TCP_POLICY),
		Policy::AllowAll(Protocol::HTTP) => Program::allow_all(&HTTP_POLICY),
		Policy::DenyAll(Protocol::TCP) => Program::deny_all(&TCP_POLICY),
		Policy::DenyAll(Protocol::HTTP) => Program::deny_all(&HTTP_POLICY),
		Policy::Bincode(s) => Program::from_bincode_raw(s.as_bytes()),
		_ => Err(io::Error::new(io::ErrorKind::Other, "missing protocol")),
	}
}

#[post("/update")]
pub async fn update(
	name: web::Data<String>,
	master: Master,
	request: web::Json<PolicyUpdate>,
) -> HttpResponse {
	if let Ok(label) = request.label.parse::<Label>() {
		if let Some(m) = MASTER_PROXY_LABEL.match_with(&label) {
			if m.get("master")
				.map(|master_name| master_name == **name)
				.unwrap_or(true)
			{
				match policy(&request.policy) {
					Ok(prog) => {
						let prog_protocol = prog.protocol();
						if let Ok(protocol) = prog_protocol.parse::<Protocol>() {
							log::info!(
								"sending {} policy: {} {}",
								protocol,
								label,
								prog.blake3_hash().unwrap()
							);
							match master
								.send(PolicyCommand(
									m.get("proxy")
										.map(InstanceSelector::Name)
										.unwrap_or(InstanceSelector::All),
									PolicyRequest::SetPolicy(protocol, prog),
								))
								.await
							{
								Ok(None) => HttpResponse::Ok().finish(),
								Ok(Some(err)) => HttpResponse::BadRequest().body(err),
								Err(err) => {
									log::warn!("{}", err);
									HttpResponse::InternalServerError().finish()
								}
							}
						} else if prog_protocol.is_empty() {
							HttpResponse::BadRequest().body("protocol is not specified in policy")
						} else {
							HttpResponse::BadRequest()
								.body(format!("unrecognized policy protocol: {}", prog_protocol))
						}
					}
					Err(err) => HttpResponse::BadRequest()
						.body(format!("failed to parse policy bincode:\n{}", err)),
				}
			} else {
				HttpResponse::BadRequest().body("policy is not for this master")
			}
		} else {
			HttpResponse::BadRequest().body("expecting label of the form <master>::<proxy>")
		}
	} else {
		HttpResponse::BadRequest().body("failed to parse label")
	}
}
