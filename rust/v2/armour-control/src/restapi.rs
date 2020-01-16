use super::ControlPlaneState;
use actix_web::{get, post, web, web::Json, HttpResponse, Responder};
use armour_api::control::*;
use bson::{bson, doc};

const ARMOUR_DB: &'static str = "armour";
const MASTERS_COL: &'static str = "masters";
const SERVICES_COL: &'static str = "services";
const POLICIES_COL: &'static str = "policies";

type State = web::Data<std::sync::Arc<ControlPlaneState>>;

#[post("/onboard-master")]
pub async fn onboard_master(
    state: State,
    request: Json<OnboardMasterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    info!("Onboarding master {:?}", request.host);

    // TODO Perform appropriate checks if necessary

    let connection = &state.db_con;

    let db = connection.database(ARMOUR_DB);
    let col = db.collection(MASTERS_COL);

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
pub async fn onboard_service(
    state: State,
    request: Json<OnboardServiceRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    info!("Onboarding service {:?}", request.label);

    let connection = &state.db_con;

    let db = connection.database(ARMOUR_DB);
    let col = db.collection(SERVICES_COL);

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

    Ok(HttpResponse::Ok().body("success".to_string()))
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
// Returns old policy if present, or new policy if not
#[post("/update-policy")]
async fn update_policy(
    state: State,
    request: Json<PolicyUpdateRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    info!("Updating policy for {:?}", request.service);

    let connection = &state.db_con;

    let db = connection.database(ARMOUR_DB);
    let col = db.collection(POLICIES_COL);

    let service = request.service.to_string();

    let service_clone = service.clone();
    let clone_policy = request.policy.clone();

    if let bson::Bson::Document(document) = bson::to_bson(&request.into_inner()).unwrap() {
        if let Ok(Some(doc)) = col.find_one(Some(doc! {"service": service}), None) {
            let current =
                bson::from_bson::<PolicyUpdateRequest>(bson::Bson::Document(doc)).unwrap();
            // To obtain the old policy:
            // let p : Program = serde_json::from_str(&current.policy).unwrap();

            let _ = col.delete_many(doc! {"service" : service_clone}, None);
            let _ = col.insert_one(document, None);
            Ok(HttpResponse::Ok().body(current.policy))
        } else {
            let _ = col.insert_one(document, None);
            Ok(HttpResponse::Ok().body(clone_policy))
        }
    } else {
        println!("Error converting the BSON object into a MongoDB document");
        Err(actix_web::Error::from(
            HttpResponse::InternalServerError().body(format!("Error inserting policy")),
        ))
    }
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
#[get("/query-policy")]
async fn query_policy(state: State, request: Json<PolicyQuery>) -> impl Responder {
    info!("Querying policy for {:?}", request.service);

    let connection = &state.db_con;

    let db = connection.database(ARMOUR_DB);
    let col = db.collection(POLICIES_COL);

    let service = request.service.clone();
    let service_clone = request.service.clone();

    if let Ok(Some(doc)) = col.find_one(Some(doc! {"service": service}), None) {
        let current =
            bson::from_bson::<PolicyUpdateRequest>(bson::Bson::Document(doc)).unwrap();
        Ok(HttpResponse::Ok().body(current.policy))
    } else {
        Err(actix_web::Error::from(
            HttpResponse::InternalServerError().body(format!("No policy for {}", service_clone)),
        ))
    }
}
