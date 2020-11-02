use actix_web::{client, delete, get, post, web, web::Json, HttpResponse};
use armour_api::control;
use armour_api::host::PolicyUpdate;
use armour_lang::{
    labels::Label, 
    policies::{DPPolicies}, 
    literals::OnboardingResult
};
use bson::doc;
use std::str::FromStr;



pub const ARMOUR_DB: &str = "armour";
pub const HOSTS_COL: &str = "hosts";
pub const SERVICES_COL: &str = "services";
pub const POLICIES_COL: &str = "policies";

pub type State = web::Data<super::ControlPlaneState>;

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

/// BEGIN

use armour_lang::{
    expressions,
    interpret::CPEnv,
    literals::{self, CPFlatLiteral, CPLiteral},
    policies_cp::{self, ObPolicy, ONBOARDING_SERVICES},
};
use futures::future::{BoxFuture, FutureExt};
use super::interpret::*;

//TODO get ride of ObPolicy and write a default OnboardingPolicy
pub struct OnboardingPolicy{
    policy: ObPolicy,
    env: Option<CPEnv>,
    //status: PolicyStatus, FIXME maybe needed to add a timeout
}

impl OnboardingPolicy{
    fn new(pol : policies_cp::OnboardingPolicy) -> Self {
        let env = Some(CPEnv::new(&pol.program));
        OnboardingPolicy {
            policy: ObPolicy::Custom(pol),
            env,
        }
    }

    fn set_policy(&mut self, p: ObPolicy) {
        //self.status.update_for_policy(&p);
        self.policy = p;
        self.env = match self.policy() {
            ObPolicy::Custom(ref pol) => Some(CPEnv::new(pol.program())),
            _ => None
        };
    }
    fn policy(&self) -> ObPolicy {
        self.policy.clone()
    }
    fn env(&self) -> &Option<CPEnv> {
        &self.env
    }
    fn evaluate_custom(//<T: std::convert::TryFrom<literals::CPLiteral> + Send + 'static>(
        &self,
        state: State,
        onboarding_data: expressions::CPExpr,//onboardingData
    ) -> BoxFuture<'static, Result<Box<literals::OnboardingResult>, expressions::Error>> {
        log::debug!("evaluting onboarding service policy");
        let now = std::time::Instant::now();
        let env = match self.env { Some(ref env) => env.clone(), _ => panic!("should never happen")}; //FIXME find a better structure than the panic 
        async move {
            let result = expressions::Expr::call(ONBOARDING_SERVICES, vec!(onboarding_data))
                .sevaluate(&state, env.clone())
                .await?;
            //let meta = env.egress().await;
            log::debug!("result ({:?}): {}", now.elapsed(), result);
            if let expressions::Expr::LitExpr(lit) = result {
                match lit {
                    CPLiteral::FlatLiteral(CPFlatLiteral::OnboardingResult(r)) => {
                        Ok(r)
                    }, 
                    _ => Err(expressions::Error::new("literal has wrong type"))
                }
            } else {
                Err(expressions::Error::new("did not evaluate to a literal"))
            }
        }
        .boxed()
    }
    fn evaluate(//<T: std::convert::TryFrom<literals::CPLiteral> + Send + 'static>(
        &self,
        state: &State,
        onboarding_data: expressions::CPExpr,//onboardingData
    ) -> BoxFuture<'static, Result<Box<literals::OnboardingResult>, expressions::Error>> {
        log::debug!("evaluting onboarding service policy");
        match self.policy {
            ObPolicy::Custom(_) => self.evaluate_custom(state.clone(), onboarding_data),
            ObPolicy::None => {
                async move {
                    Err(expressions::Error::new("onboarding is disallowed, onboarding policy needed")) 
                }.boxed()
            }
        }
    }
}

impl Default for OnboardingPolicy {
    fn default() -> Self {
        let policy = ObPolicy::onboard_none();
        let env = match policy {
            ObPolicy::Custom(ref pol) => Some(CPEnv::new(pol.program())),
            _ => None
        };
        OnboardingPolicy {
            policy,
            env,
        }
    }
}

pub const ONBOARDING_POLICY_KEY : &str = "onboarding_policy";
pub const GLOBAL_POLICY_KEY : &str = "global_policy";
pub fn onboarding_policy_label() -> Label {
    Label::from_str(ONBOARDING_POLICY_KEY).unwrap()
}
pub fn global_policy_label() -> Label {
    Label::from_str(GLOBAL_POLICY_KEY).unwrap()
}
/// END

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
        client: web::Data<client::Client>,
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let service = &request.service;
        log::info!("onboarding service: {}", service);

        //Getting current onboarding policy from db
        let ob_policy: OnboardingPolicy = {             
            let pol_col = collection(&state, POLICIES_COL);

            if let Ok(Some(doc)) = pol_col
                .find_one(Some(doc! { "label" : to_bson(&onboarding_policy_label())? }), None)
                .await
            {
                let request = bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?;
                let pol = control::OnboardingUpdateRequest::unpack(request).policy;       
                OnboardingPolicy::new(pol)
            } else {
                OnboardingPolicy::default() 
            }
        };

        //Converting OnboardServiceRequest to OnboardingData
        let onboarding_data : expressions::CPExpr = expressions::Expr::LitExpr(
            literals::CPLiteral::FlatLiteral(literals::CPFlatLiteral::OnboardingData(
                Box::new(literals::OnboardingData::new(
                    request.host.clone(),
                    request.service.clone()
            ))))
        );            
        
        //Eval policy and register specialized policies
        match ob_policy.evaluate(&state, onboarding_data).await {
            Ok(obr) => match *obr {
                OnboardingResult::Ok(id, local_pol) => {
                    let service_id = id.find_label(
                        &Label::from_str("ServiceID::**").unwrap()
                    ).ok_or(internal("Extracting service_id from id labels"))?
                    .clone(); 
                    
                    let request = control::PolicyUpdateRequest{
                        label: service_id,
                        policy: *local_pol.pol,
                        labels: control::LabelMap::default()
                    };
                        
                    policy::helper_update(client, state, request).await 
                },
                OnboardingResult::Err(e, _,_) => { Ok(internal(e)) } 
            }
            Err(e) => { Ok(internal(e.to_string())) }             
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
        client: &client::Client,
        state: &State,
        label: &Label,
        policy: &DPPolicies,
    ) -> Result<(), actix_web::Error> {
        let hosts = hosts(state, label).await?;
        log::debug!("hosts: {:?}", hosts);
        for host in hosts {
            if let Some(host_str) = host.host_str() {
                let req = PolicyUpdate {
                    label: label.clone(),
                    policy: policy.clone(),
                };
                let url = format!(
                    "https://{}:{}/policy/update",
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

    pub async fn helper_update(
        client: web::Data<client::Client>,
        state: State,
        request: control::PolicyUpdateRequest,
    ) -> Result<HttpResponse, actix_web::Error> {
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
            update_hosts(&client.into_inner(), &state, label, &request.policy).await?;
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }
    #[post("/update-global")]
    pub async fn update_global(
        state: State,
        request: Json<control::CPPolicyUpdateRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let mut request = request.into_inner();
        request.label = global_policy_label();

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
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }

    #[post("/update-onboarding")]
    pub async fn update_onboarding(
        state: State,
        request: Json<control::OnboardingUpdateRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let mut request = request.into_inner().pack();//FIXME Some issue with bson encoding, get ride of this with pack/unpack, issues with private/public ? /kind of scope
        request.label = onboarding_policy_label();
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
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }
    #[post("/update")]
    pub async fn update(
        client: web::Data<client::Client>,
        state: State,
        request: Json<control::PolicyUpdateRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
       helper_update(client, state,  request.into_inner()).await 
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
        client: web::Data<client::Client>,
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
        update_hosts(&client.into_inner(), &state, label, &DPPolicies::deny_all()).await?;
        Ok(HttpResponse::Ok().body(format!("dropped {}", res.deleted_count)))
    }

    #[delete("/drop-all")]
    async fn drop_all(
        client: web::Data<client::Client>,
        state: State,
    ) -> Result<HttpResponse, actix_web::Error> {
        log::info!("dropping all policies");
        let services = services(&state).await?;
        let client = client.into_inner();
        if collection(&state, POLICIES_COL).drop(None).await.is_ok() {
            for label in services {
                update_hosts(&client, &state, &label, &DPPolicies::deny_all()).await?;
            }
        }
        Ok(HttpResponse::Ok().body("dropped all policies"))
    }
}

pub async fn present(
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

pub fn collection(state: &State, collection: &str) -> mongodb::Collection {
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
