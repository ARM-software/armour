use actix_web::{post, web, HttpResponse};
// use actix_web::HttpRequest;
use armour_api::master::PolicyUpdate;
use armour_lang::lang::Program;
// use futures::StreamExt;
use std::sync::Arc;

type Master = web::Data<Arc<actix::Addr<super::ArmourDataMaster>>>;

#[post("/update")]
pub async fn update(
	_master: Master,
	// req: HttpRequest,
	// mut payload: web::Payload,
	request: web::Json<PolicyUpdate>,
) -> Result<HttpResponse, actix_web::Error> {
	let policy = Program::from_bincode(&request.policy)?;
	info!("protocol: {:?}", request.protocol);
	info!("policy: {}", policy.to_string());
	Ok(HttpResponse::Ok().finish())
}
