use actix_web::{post, web, HttpResponse};
// use actix_web::HttpRequest;
use armour_api::{labels::Label, master::PolicyUpdate};
use armour_lang::lang::Program;
// use futures::StreamExt;

type Master = web::Data<actix::Addr<super::ArmourDataMaster>>;

#[post("/update")]
pub async fn update(
	master: Master,
	// req: HttpRequest,
	// mut payload: web::Payload,
	request: web::Json<PolicyUpdate>,
) -> Result<HttpResponse, actix_web::Error> {
	// TODO:
	// 1. get "master", "proxy" and "protocol" components of label
	// 2. check master is self (if not error)
	// 3. add policy under "proxy::protocol" label
	// 4. if the proxy is active (launched) then forward policy
	if let Ok(label) = request.label.parse::<Label>() {
		let prog = Program::from_bincode(&request.policy)?;
		log::info!("sending policy: {} {}", label, prog.blake3_hash().unwrap());
		master
			.send(super::MasterCommand::UpdatePolicy(
				super::InstanceSelector::All, // TODO:
				Box::new(armour_api::proxy::PolicyRequest::SetPolicy(
					armour_api::proxy::Protocol::All, // TODO
					prog,
				)),
			))
			.await?;
		Ok(HttpResponse::Ok().finish())
	} else {
		Ok(HttpResponse::BadRequest().body("failed to parse label"))
	}
}
