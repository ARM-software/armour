use actix_web::{client, delete, get, post, web, web::Json, HttpResponse};
use armour_api::control;
use armour_api::host::PolicyUpdate;
use armour_lang::{labels::Label, policies::Policies};
use bson::doc;

const ARMOUR_DB: &str = "armour";
const HOSTS_COL: &str = "hosts";
const SERVICES_COL: &str = "services";
const POLICIES_COL: &str = "policies";

type State = web::Data<super::ControlPlaneState>;

pub mod host {
    use super::*;

    #[get("/list")]
    pub async fn list(state: State) -> Result<HttpResponse, actix_web::Error> {
        use futures::StreamExt;
        let col = collection(&state, HOSTS_COL);
        let mut docs = col
            .find(doc! {}, None)
            .await
            .on_err("error listing hosts")?;
        let mut s = String::new();
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let host =
                    bson::from_bson::<control::OnboardHostRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?;
                s.push_str(format!("{} ({})\n", host.label, host.host).as_str())
            }
        }
        Ok(HttpResponse::Ok().body(s))
    }

    #[post("/on-board")]
    pub async fn on_board(
        state: State,
        request: Json<control::OnboardHostRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = &request.label;
        let host = &request.host;
        log::info!("Onboarding host: {} ({})", label, host);
        let col = collection(&state, HOSTS_COL);

        // Check if the host is already there
        if present(&col, doc! { "label" : to_bson(label)? }).await? {
            Ok(internal(format!(
                r#"host label "{}" already present"#,
                label
            )))
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
        request: Json<control::OnboardHostRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = request.label.clone();
        let host = &request.host;
        log::info!("dropping host: {} ({})", label, host);

        let col = collection(&state, HOSTS_COL);
        if let bson::Bson::Document(document) = to_bson(&request.into_inner())? {
            col.delete_one(document, None)
                .await
                .on_err("error removing host from MongoDB")?;
            let col = collection(&state, SERVICES_COL);
            let filter = doc! { "label" : to_bson(&label)? };
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

    #[get("/list")]
    pub async fn list(state: State) -> Result<HttpResponse, actix_web::Error> {
        use futures::StreamExt;
        let col = collection(&state, SERVICES_COL);
        let mut docs = col
            .find(doc! {}, None)
            .await
            .on_err("error listing services")?;
        let mut s = String::new();
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let service =
                    bson::from_bson::<control::OnboardServiceRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?;
                s.push_str(format!("{} ({})\n", service.service, service.host).as_str())
            }
        }
        Ok(HttpResponse::Ok().body(s))
    }

    #[post("/on-board")]
    pub async fn on_board(
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let service = &request.service;
        log::info!("onboarding service: {}", service);
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
        log::info!("dropping service: {}", service);
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
        let hosts_col = collection(state, HOSTS_COL);
        let mut hosts = BTreeSet::new();
        // find hosts for service
        let mut docs = collection(state, SERVICES_COL)
            .find(doc! { "service" : to_bson(label)? }, None)
            .await
            .on_err("error notifying hosts")?;
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let host =
                    bson::from_bson::<control::OnboardServiceRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?
                        .host;
                // find host
                if let Ok(Some(doc)) = hosts_col
                    .find_one(Some(doc! { "label" : to_bson(&host)? }), None)
                    .await
                {
                    hosts.insert(
                        bson::from_bson::<control::OnboardHostRequest>(bson::Bson::Document(doc))
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
            if let Some(host_str) = host.host_str() {
                let req = PolicyUpdate {
                    label: label.clone(),
                    policy: policy.clone(),
                };
                let url = format!(
                    "http://{}:{}/policy/update",
                    host_str,
                    host.port().unwrap_or(8090)
                );
                match client.post(url).send_json(&req).await {
                    Ok(res) => {
                        if res.status().is_success() {
                            log::info!("pushed policy to {}", host)
                        } else {
                            log::info!("failed to push policy to {}", host)
                        }
                    }
                    Err(err) => log::warn!("{}: {}", host, err),
                }
            } else {
                log::warn!("failed to contact host: {}", host)
            }
        }
        Ok(())
    }

    #[get("/list")]
    pub async fn list(state: State) -> Result<HttpResponse, actix_web::Error> {
        use futures::StreamExt;
        let col = collection(&state, POLICIES_COL);
        let mut docs = col
            .find(doc! {}, None)
            .await
            .on_err("error listing policies")?;
        let mut s = String::new();
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let policy =
                    bson::from_bson::<control::PolicyUpdateRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?;
                let pol = if policy.policy.is_allow_all() {
                    "(allow all)"
                } else if policy.policy.is_deny_all() {
                    "(deny all)"
                } else {
                    ""
                };
                s.push_str(format!("{}{}\n", policy.label, pol).as_str())
            }
        }
        Ok(HttpResponse::Ok().body(s))
    }

    #[post("/update")]
    pub async fn update(
        state: State,
        request: Json<control::PolicyUpdateRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let request = request.into_inner();
        let label = &request.label.clone();
        log::info!(r#"updating policy for label "{}""#, label);

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
            // push policy to hosts
            update_hosts(&state, label, &request.policy).await?;
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }

    #[get("/query")]
    async fn query(
        state: State,
        request: Json<control::PolicyQueryRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = &request.label;
        log::info!("querying policy for {}", label);
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
        log::info!("dropping policy for {}", label);
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
        log::info!("dropping all policies");
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

impl<T> OnErr<T, bson::de::Error> for bson::de::Result<T> {}
impl<T> OnErr<T, bson::ser::Error> for bson::ser::Result<T> {}
impl<T> OnErr<T, mongodb::error::Error> for mongodb::error::Result<T> {}
