use bson::{bson, doc};
use serde::{Deserialize, Serialize};

// use serde_json;

use actix_web::{get, post, web, web::Json, HttpResponse, Responder};
// use redis::Commands;

use armour_policy::lang::Program;

use super::ControlPlaneState;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OnboardMasterRequest {
	pub host: String,        // FIXME change types as needed
	pub credentials: String, // FIXME change types as needed
}

#[post("/onboard-master")]
pub fn onboard_master(
	state: web::Data<ControlPlaneState>,
	request: Json<OnboardMasterRequest>,
) -> impl Responder {
	info!("Onboarding master {:?}", request.host);

	// TODO Perform appropriate checks if necessary

	let connection = &state.db_con;

	let db = connection.database("armour");
	let col = db.collection("masters");

	// Check if the master is already there
	let filter = doc! { "host" : &request.host };

	let result: Vec<Result<bson::Document, mongodb::error::Error>> =
		col.find(filter, None).unwrap().collect();
	if result.len() != 0 {
		error!("Master already preseent in {:?}", &request.host);
		// FIXME raise an error
	}

	if let bson::Bson::Document(document) = bson::to_bson(&request.into_inner()).unwrap() {
		col.insert_one(document, None).unwrap(); // Insert into a MongoDB collection
	} else {
		println!("Error converting the BSON object into a MongoDB document");
	}

	HttpResponse::Ok().body("success".to_string())
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceRequest {
	pub label: String,  // FIXME
	pub master: String, // FIXME
}

#[post("/onboard-service")]
pub fn onboard_service(
	state: web::Data<ControlPlaneState>,
	request: Json<OnboardServiceRequest>,
) -> impl Responder {
	info!("Onboarding service {:?}", request.label);

	let connection = &state.db_con;

	let db = connection.database("armour");
	let col = db.collection("services");

	if let bson::Bson::Document(document) = bson::to_bson(&request.into_inner()).unwrap() {
	    let res = col.insert_one(document, None).unwrap(); // Insert into a MongoDB collection
	    info!("Result of insertion is: {:?}", res.inserted_id );
	    let doc = col.find_one(Some(doc! {"_id" : res.inserted_id}), None)
		.expect("Document not found");
	    info!("Is it there? {:?}", doc);
	} else {
		println!("Error converting the BSON object into a MongoDB document");
	}

	HttpResponse::Ok().body("success".to_string())
}

#[derive(Serialize, Deserialize)]
pub struct PolicyUpdateRequest {
	pub service: String, // FIXME
	pub policy: Program,
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
#[post("/update-policy")]
fn update_policy(
	_state: web::Data<ControlPlaneState>,
	request: Json<PolicyUpdateRequest>,
) -> impl Responder {
	info!("Updating policy for {:?}", request.service);

	HttpResponse::Ok().body("Policy updater")
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQuery {
	pub service: String, // FIXME
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
#[get("/query-policy")]
fn query_policy(
	_state: web::Data<ControlPlaneState>,
	request: Json<PolicyQuery>,
) -> impl Responder {
	info!("Querying policy for {:?}", request.service);

	HttpResponse::Ok().body("Policy updater")
}

// #[get("/index")]
// fn index() -> impl Responder {
// 	info!("Index");

// 	HttpResponse::Ok().body(serde_json::to_string(&OnboardMasterRequest {
// 		host: "localhost".to_string(),
// 		credentials: "no-creds".to_string(),
// 	})
// 	.unwrap())
// }
