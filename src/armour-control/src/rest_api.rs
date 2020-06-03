use actix_web::{client, delete, get, post, web, web::Json, HttpResponse};
use armour_api::control;
use armour_api::master::PolicyUpdate;
use armour_lang::{labels::Label, policies::Policies};
use bson::doc;

const ARMOUR_DB: &str = "armour";
const MASTERS_COL: &str = "masters";
const SERVICES_COL: &str = "services";
const POLICIES_COL: &str = "policies";

type State = web::Data<super::ControlPlaneState>;

pub mod master {
    use super::*;

    #[post("/on-board")]
    pub async fn on_board(
        state: State,
        request: Json<control::OnboardMasterRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let master = &request.master;
        let host = &request.host;
        log::info!("Onboarding master: {} ({})", master, host);
        let col = collection(&state, MASTERS_COL);

        // Check if the master is already there
        if present(&col, doc! { "master" : to_bson(master)? }).await? {
            Ok(internal(format!(r#"Master "{}" already present"#, master)))
        } else if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
            col.insert_one(document, None)
                .await
                .on_err("error inserting in MongoDB")?;
            Ok(HttpResponse::Ok().body("success"))
        } else {
            Ok(internal("error extracting document"))
        }
    }

    #[delete("/drop")]
    pub async fn drop(
        state: State,
        request: Json<control::OnboardMasterRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let master = request.master.clone();
        let host = &request.host;
        log::info!("Dropping master: {} ({})", master, host);

        let col = collection(&state, MASTERS_COL);
        if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
            col.delete_one(document, None)
                .await
                .on_err("error removing master from MongoDB")?;
            let col = collection(&state, SERVICES_COL);
            let filter = doc! { "master" : to_bson(&master)? };
            col.delete_many(filter, None)
                .await
                .on_err("error removing services from MongoDB")?;

            Ok(HttpResponse::Ok().body("success"))
        } else {
            Ok(internal("error extracting document"))
        }
    }
}

pub mod service {
    use super::*;

    #[post("/on-board")]
    pub async fn on_board(
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let service = &request.service;
        log::info!("Onboarding service: {}", service);
        let col = collection(&state, SERVICES_COL);

        // Check if the service is already there
        if present(&col, doc! { "service" : to_bson(service)? }).await? {
            Ok(internal(format!("service label in use {}", service)))
        } else if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
            col.insert_one(document, None) // Insert into a MongoDB collection
                .await
                .on_err("error inserting in MongoDB")?;
            Ok(HttpResponse::Ok().body("success"))
        } else {
            Ok(internal("error extracting document"))
        }
    }

    #[delete("/drop")]
    pub async fn drop(
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let service = &request.service;
        log::info!("Dropping service: {}", service);
        let col = collection(&state, SERVICES_COL);

        if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
            col.delete_one(document, None) // Insert into a MongoDB collection
                .await
                .on_err("error inserting in MongoDB")?;
            Ok(HttpResponse::Ok().body("success"))
        } else {
            Ok(internal("error extracting document"))
        }
    }
}

pub mod policy {
    use super::*;
    use std::collections::BTreeSet;

    async fn services(state: &State) -> Result<BTreeSet<Label>, actix_web::Error> {
        use futures::StreamExt;
        let mut services = BTreeSet::new();
        let mut docs = collection(state, POLICIES_COL)
            .find(doc! {}, None)
            .await
            .on_err("error finding services")?;
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let label =
                    bson::from_bson::<control::PolicyUpdateRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?
                        .label;
                services.insert(label);
            }
        }
        Ok(services)
    }

    async fn hosts(state: &State, label: &Label) -> Result<BTreeSet<url::Url>, actix_web::Error> {
        use futures::StreamExt;
        let masters_col = collection(state, MASTERS_COL);
        let mut hosts = BTreeSet::new();
        // find masters for service
        let mut docs = collection(state, SERVICES_COL)
            .find(doc! { "service" : to_bson(label)? }, None)
            .await
            .on_err("Error notifying masters")?;
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let master =
                    bson::from_bson::<control::OnboardServiceRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?
                        .master;
                // find host for master
                if let Ok(Some(doc)) = masters_col
                    .find_one(Some(doc! { "master" : to_bson(&master)? }), None)
                    .await
                {
                    hosts.insert(
                        bson::from_bson::<control::OnboardMasterRequest>(bson::Bson::Document(doc))
                            .on_err("Bson conversion error")?
                            .host,
                    );
                }
            }
        }
        Ok(hosts)
    }

    pub async fn update_hosts(
        state: &State,
        label: &Label,
        policy: &Policies,
    ) -> Result<(), actix_web::Error> {
        let client = client::Client::default();
        let hosts = hosts(state, label).await?;
        log::debug!("hosts: {:?}", hosts);
        for host in hosts {
            let req = PolicyUpdate {
                label: label.clone(),
                policy: policy.clone(),
            };
            match client
                .post(format!("http://{}/policy/update", host))
                .send_json(&req)
                .await
            {
                Ok(res) => {
                    if res.status().is_success() {
                        log::info!("pushed policy to {}", host)
                    } else {
                        log::info!("failed to push policy to {}", host)
                    }
                }
                Err(err) => log::warn!("{}: {}", host, err),
            }
        }
        Ok(())
    }

    // FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
    #[post("/update")]
    pub async fn update(
        state: State,
        request: Json<control::PolicyUpdateRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let request = request.into_inner();
        let label = &request.label.clone();
        log::info!(r#"Updating policy for label "{}""#, label);

        if let bson::Bson::Document(document) = to_bson(&request)? {
            // update policy in database
            let col = collection(&state, POLICIES_COL);
            let filter = doc! { "label" : to_bson(label)? };
            col.delete_many(filter, None)
                .await
                .on_err("error removing old policies")?;
            col.insert_one(document, None)
                .await
                .on_err("error inserting new policy")?;
            // push policy to masters
            update_hosts(&state, label, &request.policy).await?;
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("Error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }

    // FIXME: Not clear that we need shared data in the server. I think I prefer to have a DB.
    #[get("/query")]
    async fn query(
        state: State,
        request: Json<control::PolicyQueryRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = &request.label;
        log::info!("Querying policy for {}", label);
        let col = collection(&state, POLICIES_COL);
        if let Ok(Some(doc)) = col
            .find_one(Some(doc! { "label" : label.to_string() }), None)
            .await
        {
            let current =
                bson::from_bson::<control::PolicyUpdateRequest>(bson::Bson::Document(doc))
                    .on_err("Bson conversion error")?;
            Ok(HttpResponse::Ok().json(current))
        } else {
            Ok(HttpResponse::NotFound().body(format!("no policy for {}", label)))
        }
    }

    #[delete("/drop")]
    async fn drop(
        state: State,
        request: Json<control::PolicyQueryRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = &request.label;
        log::info!("Dropping policy for {}", label);
        let col = collection(&state, POLICIES_COL);
        let res = col
            .delete_one(doc! { "label" : label.to_string() }, None)
            .await
            .on_err("failed to drop policy")?;
        update_hosts(&state, label, &Policies::deny_all()).await?;
        Ok(HttpResponse::Ok().body(format!("dropped {}", res.deleted_count)))
    }

    #[delete("/drop-all")]
    async fn drop_all(state: State) -> Result<HttpResponse, actix_web::Error> {
        log::info!("Dropping all policies");
        let services = services(&state).await?;
        if collection(&state, POLICIES_COL).drop(None).await.is_ok() {
            for label in services {
                update_hosts(&state, &label, &Policies::deny_all()).await?;
            }
        }
        Ok(HttpResponse::Ok().body("dropped all policies"))
    }
}

async fn present(
    col: &mongodb::Collection,
    filter: impl Into<Option<bson::Document>>,
) -> Result<bool, actix_web::Error> {
    use futures::StreamExt;
    Ok(col
        .find(filter, None)
        .await
        .on_err("MongoDB query error")?
        .next()
        .await
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
