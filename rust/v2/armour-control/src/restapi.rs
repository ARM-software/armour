use super::ControlPlaneState;
use actix_web::{get, post, web, web::Json, HttpResponse, Responder};
use armour_api::control::*;
use bson::{bson, doc};

type State = web::Data<std::sync::Arc<ControlPlaneState>>;

#[post("/onboard-master")]
pub async fn onboard_master(
	state: State,
	request: Json<OnboardMasterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
	info!("Onboarding master {:?}", request.host);

	// TODO Perform appropriate checks if necessary

	let connection = &state.db_con;

	let db = connection.database("armour");
	let col = db.collection("masters");

	// Check if the master is already there
	let filter = doc! { "host" : bson::to_bson(&request.host).unwrap() };

	let result: Vec<Result<bson::Document, mongodb::error::Error>> =
		col.find(filter, None).unwrap().collect();
	if !result.is_empty() {
		return Err(actix_web::Error::from(
			HttpResponse::InternalServerError()
				.body(format!("Master already present in {:?}", &request.host)),
		));
	}

	if let bson::Bson::Document(document) = bson::to_bson(&request.into_inner()).unwrap() {
		col.insert_one(document, None).unwrap(); // Insert into a MongoDB collection
	} else {
		println!("Error converting the BSON object into a MongoDB document");
	}

	Ok(HttpResponse::Ok().body("success".to_string()))
}

#[post("/onboard-service")]
pub async fn onboard_service(state: State, request: Json<OnboardServiceRequest>) -> impl Responder {
	info!("Onboarding service {:?}", request.label);

	let connection = &state.db_con;

	let db = connection.database("armour");
	let col = db.collection("services");

	if let bson::Bson::Document(document) = bson::to_bson(&request.into_inner()).unwrap() {
		let res = col.insert_one(document, None).unwrap(); // Insert into a MongoDB collection
		info!("Result of insertion is: {:?}", res.inserted_id);
		let doc = col
			.find_one(Some(doc! {"_id" : res.inserted_id}), None)
			.expect("Document not found");
		info!("Is it there? {:?}", doc);
	} else {
		println!("Error converting the BSON object into a MongoDB document");
	}

	HttpResponse::Ok().body("success".to_string())
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
#[post("/update-policy")]
async fn update_policy(_state: State, request: Json<PolicyUpdateRequest>) -> impl Responder {
	info!("Updating policy for {:?}", request.service);

	HttpResponse::Ok().body("Policy updater")
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
#[get("/query-policy")]
async fn query_policy(_state: State, request: Json<PolicyQuery>) -> impl Responder {
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
