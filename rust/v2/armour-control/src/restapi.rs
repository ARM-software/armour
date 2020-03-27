use super::ControlPlaneState;
use actix_web::{get, post, web, web::Json, HttpResponse};
use armour_api::control::*;
use bson::{bson, doc};

const ARMOUR_DB: &str = "armour";
const MASTERS_COL: &str = "masters";
const SERVICES_COL: &str = "services";
const POLICIES_COL: &str = "policies";

type State = web::Data<ControlPlaneState>;

#[post("/onboard-master")]
pub async fn onboard_master(
    state: State,
    request: Json<OnboardMasterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Onboarding master {:?}", request.host);

    let connection = &state.db_con;
    let db = connection.database(ARMOUR_DB);
    let col = db.collection(MASTERS_COL);

    // Check if the master is already there
    let result: Vec<Result<bson::Document, mongodb::error::Error>> = col
        .find(doc! { "host" : to_bson(&request.host)? }, None)
        .map_err(|_| internal("MongoDB query error"))?
        .collect();

    if !result.is_empty() {
        Ok(internal(format!(
            "Master already present for {:}",
            request.host
        )))
    } else if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        col.insert_one(document, None)
            .map_err(|_| internal("Error inserting in MongoDB"))?;
        Ok(HttpResponse::Ok().body("success"))
    } else {
        Ok(internal("Error extracting document"))
    }
}

#[post("/onboard-service")]
pub async fn onboard_service(
    state: State,
    request: Json<OnboardServiceRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Onboarding service {:?}", request.label);

    let connection = &state.db_con;
    let db = connection.database(ARMOUR_DB);
    let col = db.collection(SERVICES_COL);

    // Check if the service is already there
    let result: Vec<Result<bson::Document, mongodb::error::Error>> = col
        .find(doc! { "label" : to_bson(&request.label)? }, None)
        .map_err(|_| internal("MongoDB query error"))?
        .collect();

    if !result.is_empty() {
        Ok(internal(format!(
            "Service label in use {:?}",
            request.label
        )))
    } else if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        col.insert_one(document, None) // Insert into a MongoDB collection
            .map_err(|_| internal("Error inserting in MongoDB"))?;
        Ok(HttpResponse::Ok().body("success"))
    } else {
        Ok(internal("Error extracting document"))
    }
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
// Returns old policy if present, or new policy if not
#[post("/update-policy")]
async fn update_policy(
    state: State,
    request: Json<PolicyUpdateRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Updating policy for {:?}", request.service);
    let connection = &state.db_con;
    let db = connection.database(ARMOUR_DB);
    let col = db.collection(POLICIES_COL);

    let service = request.service.to_string();

    let service_clone = service.clone();
    let clone_policy = request.policy.clone();

    if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        if let Ok(Some(doc)) = col.find_one(Some(doc! {"service": service}), None) {
            let current = bson::from_bson::<PolicyUpdateRequest>(bson::Bson::Document(doc))
                .map_err(|_| internal("Error inserting policy"))?;
            // To obtain the old policy:
            // let p : Program = serde_json::from_str(&current.policy).unwrap();

            col.delete_many(doc! {"service" : service_clone}, None)
                .map_err(|_| internal("Error removing old policies"))?;
            col.insert_one(document, None)
                .map_err(|_| internal("Error inserting new policy"))?;
            Ok(HttpResponse::Ok().body(current.policy))
        } else {
            col.insert_one(document, None)
                .map_err(|_| internal("Error inserting new policy"))?;
            Ok(HttpResponse::Ok().body(clone_policy))
        }
    } else {
        log::warn!("Error converting the BSON object into a MongoDB document");
        Ok(internal("Error inserting policy"))
    }
}

// FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
#[get("/query-policy")]
async fn query_policy(
    state: State,
    request: Json<PolicyQueryRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    log::info!("Querying policy for {:?}", request.service);

    let connection = &state.db_con;
    let db = connection.database(ARMOUR_DB);
    let col = db.collection(POLICIES_COL);

    let service = request.service.clone();
    let service_clone = request.service.clone();

    if let Ok(Some(doc)) = col.find_one(Some(doc! {"service": service}), None) {
        let current = bson::from_bson::<PolicyUpdateRequest>(bson::Bson::Document(doc))
            .map_err(|_| internal("Bson conversion error"))?;
        Ok(HttpResponse::Ok().json(current.policy))
    } else {
        Ok(internal(format!("No policy for {}", service_clone)))
    }
}

fn internal<B: Into<actix_web::body::Body>>(b: B) -> HttpResponse {
    HttpResponse::InternalServerError().body(b)
}

pub fn to_bson<T: ?Sized>(value: &T) -> Result<bson::Bson, actix_web::Error>
where
    T: serde::Serialize,
{
    bson::to_bson(value).map_err(|_| internal("Bson conversion error").into())
}
