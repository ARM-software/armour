use super::ControlPlaneState;
use actix_web::{delete, get, post, web, web::Json, HttpResponse};
use armour_api::control::*;
use bson::{bson, doc};

const ARMOUR_DB: &str = "armour";
const MASTERS_COL: &str = "masters";
const SERVICES_COL: &str = "services";
const POLICIES_COL: &str = "policies";

type State = web::Data<ControlPlaneState>;

#[post("/on-board-master")]
pub async fn on_board_master(
    state: State,
    request: Json<OnboardMasterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let master = &request.master;
    let host = &request.host;
    log::info!("Onboarding master: {} ({})", master, host);
    let col = collection(&state, MASTERS_COL);

    // Check if the master is already there
    if present(&col, doc! { "master" : to_bson(master)? })? {
        Ok(internal(format!(r#"Master "{}" already present"#, master)))
    } else if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        col.insert_one(document, None)
            .on_err("Error inserting in MongoDB")?;
        Ok(HttpResponse::Ok().body("success"))
    } else {
        Ok(internal("Error extracting document"))
    }
}

#[delete("/drop-master")]
pub async fn drop_master(
    state: State,
    request: Json<OnboardMasterRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let master = &request.master;
    let host = &request.host;
    log::info!("Dropping master: {} ({})", master, host);
    let col = collection(&state, MASTERS_COL);

    if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        col.delete_one(document, None)
            .on_err("Error removing from MongoDB")?;
        Ok(HttpResponse::Ok().body("success"))
    } else {
        Ok(internal("Error extracting document"))
    }
}

#[post("/on-board-service")]
pub async fn on_board_service(
    state: State,
    request: Json<OnboardServiceRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let service = &request.service;
    log::info!("Onboarding service: {}", service);
    let col = collection(&state, SERVICES_COL);

    // Check if the service is already there
    if present(&col, doc! { "service" : to_bson(service)? })? {
        Ok(internal(format!("Service label in use {}", service)))
    } else if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        col.insert_one(document, None) // Insert into a MongoDB collection
            .on_err("Error inserting in MongoDB")?;
        Ok(HttpResponse::Ok().body("success"))
    } else {
        Ok(internal("Error extracting document"))
    }
}

#[delete("/drop-service")]
pub async fn drop_service(
    state: State,
    request: Json<OnboardServiceRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let service = &request.service;
    log::info!("Dropping service: {}", service);
    let col = collection(&state, SERVICES_COL);

    if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        col.delete_one(document, None) // Insert into a MongoDB collection
            .on_err("Error inserting in MongoDB")?;
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
    let service = &request.service.clone();
    log::info!("Updating policy for {}", service);
    let policy = request.policy.to_string();

    if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
        let col = collection(&state, POLICIES_COL);
        let filter = doc! { "service" : to_bson(service)? };
        if let Ok(Some(doc)) = col.find_one(Some(filter.clone()), None) {
            let current = bson::from_bson::<PolicyUpdateRequest>(bson::Bson::Document(doc))
                .on_err("Error inserting policy")?;
            // To obtain the old policy:
            // let p : Program = serde_json::from_str(&current.policy).unwrap();

            col.delete_many(filter, None)
                .on_err("Error removing old policies")?;
            col.insert_one(document, None)
                .on_err("Error inserting new policy")?;
            Ok(HttpResponse::Ok().body(current.policy))
        } else {
            col.insert_one(document, None)
                .on_err("Error inserting new policy")?;
            Ok(HttpResponse::Ok().body(policy))
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
    let service = &request.service;
    log::info!("Querying policy for {}", service);
    let col = collection(&state, POLICIES_COL);
    if let Ok(Some(doc)) = col.find_one(Some(doc! { "service" : service.to_string() }), None) {
        let current = bson::from_bson::<PolicyUpdateRequest>(bson::Bson::Document(doc))
            .on_err("Bson conversion error")?;
        Ok(HttpResponse::Ok().json(current.policy))
    } else {
        Ok(internal(format!("No policy for {}", service)))
    }
}

fn present(
    col: &mongodb::Collection,
    filter: impl Into<Option<bson::Document>>,
) -> Result<bool, actix_web::Error> {
    Ok(col
        .find(filter, None)
        .on_err("MongoDB query error")?
        .next()
        .is_some())
}

fn collection(state: &State, collection: &str) -> mongodb::Collection {
    state.db_con.database(ARMOUR_DB).collection(collection)
}

pub fn to_bson<T: ?Sized>(value: &T) -> Result<bson::Bson, actix_web::Error>
where
    T: serde::Serialize,
{
    bson::to_bson(value).on_err("Bson conversion error")
}

fn internal<B: Into<actix_web::body::Body>>(b: B) -> HttpResponse {
    HttpResponse::InternalServerError().body(b)
}

trait OnErr<T, E>
where
    Self: Into<Result<T, E>>,
{
    fn on_err<B: Into<actix_web::body::Body>>(self, b: B) -> Result<T, actix_web::Error> {
        self.into().map_err(|_| internal(b).into())
    }
}

impl<T> OnErr<T, bson::DecoderError> for bson::DecoderResult<T> {}
impl<T> OnErr<T, bson::EncoderError> for bson::EncoderResult<T> {}
impl<T> OnErr<T, mongodb::error::Error> for mongodb::error::Result<T> {}
