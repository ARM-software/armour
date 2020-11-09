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
use std::sync::Arc;



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
    policies::{self, ONBOARDING_SERVICES},
};
use futures::future::{BoxFuture, FutureExt};
use super::interpret::*;
use super::specialize::{compile_egress, compile_ingress};

//TODO get ride of ObPolicy and write a default OnboardingPolicy
pub struct OnboardingPolicy{
    policy: policies::OnboardingPolicy,
    env: CPEnv,
    //status: PolicyStatus, FIXME maybe needed to add a timeout
}

impl OnboardingPolicy{
    fn new(pol : policies::OnboardingPolicy) -> Self {
        let env = CPEnv::new(&pol.program);
        OnboardingPolicy {
            policy: pol,
            env,
        }
    }

    fn set_policy(&mut self, p: policies::OnboardingPolicy) {
        //self.status.update_for_policy(&p);
        self.policy = p;
        self.env = CPEnv::new(self.policy.program());
    }
    fn policy(&self) -> policies::OnboardingPolicy {
        self.policy.clone()
    }
    fn env(&self) -> &CPEnv {
        &self.env
    }

    fn evaluate(//<T: std::convert::TryFrom<literals::CPLiteral> + Send + 'static>(
        &self,
        state: State,
        onboarding_data: expressions::CPExpr,//onboardingData
    ) -> BoxFuture<'static, Result<Box<literals::OnboardingResult>, expressions::Error>> {
        log::debug!("evaluting onboarding service policy");
        let now = std::time::Instant::now();
        let env =self.env.clone(); 

        async move {
            let result = expressions::Expr::call(ONBOARDING_SERVICES, vec!(onboarding_data))
                .sevaluate(Arc::new(state), env.clone())
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
}

impl Default for OnboardingPolicy {
    fn default() -> Self {
        let raw_pol = "
            fn onboarding_policy(od: OnboardingData) -> OnboardingResult {
                OnboardingResult::ErrStr(\"Onboarding disabled by default, update the onboarding policy first.\")
            }
        ";
        let policy = policies::OnboardingPolicy::from_buf(raw_pol).unwrap();
        let env = CPEnv::new(policy.program());
        OnboardingPolicy { policy, env }
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

    pub async fn get_onboarding_policy(state: &State) -> Result<OnboardingPolicy, actix_web::Error>{
        let pol_col = collection(&state, POLICIES_COL);

        if let Ok(Some(doc)) = pol_col
            .find_one(Some(doc! { "label" : to_bson(&onboarding_policy_label())? }), None)
            .await
        {
            let request = bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc))
                    .on_err("Bson conversion error")?;
            let pol = control::OnboardingUpdateRequest::unpack(request).policy;       
            Ok(OnboardingPolicy::new(pol))
        } else {
            Ok(OnboardingPolicy::default())
        }
    }

    pub async fn helper_on_board(
        state: &State,
        request: control::OnboardServiceRequest,
    ) -> Result<Result<(Label, control::PolicyUpdateRequest, control::PolicyUpdateRequest), String>, actix_web::Error> {
        let service = &request.service;
        log::info!("onboarding service: {}", service);

        //Getting current onboarding policy from db
        let ob_policy: OnboardingPolicy = get_onboarding_policy(state).await?;

        //Converting OnboardServiceRequest to OnboardingData
        let onboarding_data : expressions::CPExpr = expressions::Expr::LitExpr(
            literals::CPLiteral::FlatLiteral(literals::CPFlatLiteral::OnboardingData(
                Box::new(literals::OnboardingData::new(
                    request.host.clone(),
                    request.service.clone(),
                    match request.tmp_dpid { Some(x) => x.port(), _ => None}
            ))))
        );            

        //Eval policy and register specialized policies
        Ok(match ob_policy.evaluate(state.clone(), onboarding_data).await {
            Ok(obr) => match *obr {
                OnboardingResult::Ok(id, local_pol) => {
                    let service_id = id.find_label(
                        &Label::from_str("ServiceID::**").unwrap()
                    ).ok_or(internal("Extracting service_id from id labels"))?
                    .clone(); 
                    
                    Ok((
                        service_id.clone(),
                        control::PolicyUpdateRequest{
                            label: service_id.clone(),
                            policy: *local_pol.0.pol,
                            labels: control::LabelMap::default()
                        },
                        control::PolicyUpdateRequest{
                            label: service_id,
                            policy: *local_pol.1.pol, 
                            labels: control::LabelMap::default()
                        }
                    ))
                },
                OnboardingResult::Err(e, _,_) => { Err(e) } 
            }
            Err(e) => { Err(e.to_string()) }             
        })
    }

    #[post("/on-board")]
    pub async fn on_board(
        client: web::Data<client::Client>,
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = request.service.clone();
        match helper_on_board(&state, request.into_inner()).await? {
            Ok((service_id, ingress_req, egress_req)) =>{
                match label.get_string(0) {
                    Some(s) if s == "Egress".to_string() =>{
                        policy::save_policy(client.clone(), state.clone(), egress_req).await?;
                        Ok(HttpResponse::Ok().json(control::OnboardServiceResponse{
                            service_id: service_id,
                        }))
                    },
                   Some(s) if s == "Ingress".to_string() =>{
                        policy::save_policy(client.clone(), state.clone(), ingress_req).await?;
                        Ok(HttpResponse::Ok().json(control::OnboardServiceResponse{
                            service_id: service_id,
                        }))
                    },
                    _ => Ok(internal(format!("this neither an ingress nor an egress proxy")))
                }
            }, 
            Err(s)=> Ok(internal(s))  
        }
    }

    #[delete("/drop")]
    pub async fn drop(
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let mut request = request.into_inner();

        //request.service.prefix("Service".to_string());
        if let Some(dpid) = request.tmp_dpid {
            let service = match dpid.find_label(&Label::from_str("ServiceID::**").unwrap()) {
                Some(l) => l.clone(),
                _ =>  return Ok(internal("error no global id provided"))
            };

            log::info!("dropping service: {}", service);
            let col = collection(&state, SERVICES_COL);

            col.delete_one(doc!{"service": service.to_string()}, None) // Insert into a MongoDB collection
                .await
                .on_err("error inserting in MongoDB")?;
            Ok(HttpResponse::Ok().body("success"))
        } else { 
            Ok(internal("error no global id provided"))
        }
        //if let bson::Bson::Document(document) = to_bson(&request)? {
        //    col.delete_one(document, None) // Insert into a MongoDB collection
        //        .await
        //        .on_err("error inserting in MongoDB")?;
        //    Ok(HttpResponse::Ok().body("success"))
        //} else {
        //    Ok(internal("error extracting document"))
        //}
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

    async fn services_full(state: &State) -> Result<Vec<control::POnboardServiceRequest>, actix_web::Error> {
        use futures::StreamExt;
        let mut services = Vec::new();
        let mut docs = collection(state, SERVICES_COL)
            .find(doc! {}, None)
            .await
            .on_err("error finding services")?;
        while let Some(doc) = docs.next().await {
            if let Ok(doc) = doc {
                let service =
                    bson::from_bson::<control::POnboardServiceRequest>(bson::Bson::Document(doc))
                        .on_err("Bson conversion error")?;
                services.push(service);
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
                let host = Label::from_str("Host::<<host>>").unwrap().match_with(&host).unwrap().get_label("host").unwrap().clone();

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
        local_label: &Label,
        policy: &DPPolicies,
    ) -> Result<(), actix_web::Error> {
        let hosts = hosts(state, label).await?;
        
        for host in hosts {
            if let Some(host_str) = host.host_str() {
                let req = PolicyUpdate {
                    label: local_label.clone(),
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

    pub async fn save_policy(
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
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }
    pub async fn helper_update(
        client: web::Data<client::Client>,
        state: State,
        local_label: &Label,
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
            update_hosts(&client.into_inner(), &state, label, local_label, &request.policy).await?;
            Ok(HttpResponse::Ok().finish())
        } else {
            log::warn!("error converting the BSON object into a MongoDB document");
            Ok(internal("error inserting policy"))
        }
    }
    #[post("/update-global")]
    pub async fn update_global(
        client: web::Data<client::Client>,
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

            log::info!("global policy has been updated");
            //by default all onboarding services are concerned
            let selector = &request.selector.unwrap_or(Label::from_str("ServiceID::**").unwrap());
            log::info!("propagating to proxies according to selector: {}", selector);    
            let services = services_full(&state).await?.into_iter().filter(|service|
                service.service_id.has_label(&selector)
            );
            let ingress_selector = Label::from_str("Service::Ingress::**").unwrap();
            let egress_selector = Label::from_str("Service::Egress::**").unwrap();
            let global_policy = request.policy.clone();
            for service in services {
                let local_label = get_local_service_label(&state, &service.service).await?;

                if service.service_id.has_label(&ingress_selector){
                    log::info!("updating ingress policy for {}", service.service);
                    let local_pol = compile_ingress(
                        Arc::new(state.clone()), 
                        global_policy.clone(), 
                        policies::ALLOW_REST_REQUEST, //TODO only one main fucntion is supported...
                        &service.service_id //FIXME service_id has no port inside since remove before storing in DB due to bson error
                    ).await.map_err(|e| internal(e.to_string()))?;
                    

                    helper_update(
                        client.clone(),
                        state.clone(),
                        &local_label,
                        control::PolicyUpdateRequest{
                            label: service.service.clone(),
                            policy: local_pol,
                            labels: request.labels.clone(),
                        }
                    ).await?;
                } else if service.service_id.has_label(&egress_selector) {                        
                    log::info!("updating egress policy for {}", service.service);
                    let local_pol = compile_egress(
                        Arc::new(state.clone()), 
                        global_policy.clone(), 
                        policies::ALLOW_REST_RESPONSE, //TODO only one main fucntion is supported...
                        &service.service_id //FIXME service_id has no port inside since remove before storing in DB due to bson error
                    ).await.map_err(|e| internal(e.to_string()))?;
                    

                    helper_update(
                        client.clone(),
                        state.clone(),
                        &local_label,
                        control::PolicyUpdateRequest{
                            label: service.service.clone(),
                            policy: local_pol,
                            labels: request.labels.clone(),
                        }
                    ).await?;
                } else {
                    log::info!("global policy update not propagated to proxy {}: it is neither an ingress nor an egress proxy", service.service);
                }
            }
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

    async fn get_local_service_label(
        state: &State, 
        label: &Label
    ) -> Result<Label, actix_web::Error> {
        if let Some(doc) = collection(state, SERVICES_COL)
            .find_one(doc! { "service": to_bson(label)?}, None)
            .await
            .on_err("error finding services")?{

            let service = bson::from_bson::<control::POnboardServiceRequest>(bson::Bson::Document(doc))
                    .on_err("Bson conversion error")?;

            let local_label = match service.service_id.find_label(&Label::from_str("Service::**").unwrap()) {
                Some(l) => l.clone(),
                _ =>  return Err(internal(format!("get_local_service_label failed for {}: no local id specified", label)).into())
            };                
            let local_label = Label::from_str("Service::<<service>>").unwrap().match_with(&local_label).unwrap().get_label("service").unwrap().clone();
            Ok(local_label)
        } else {
            Err(internal(format!("get_local_service_label failed for {}: no related service in DB", label)).into())
        }
    }

    #[post("/update")]
    pub async fn update(
        client: web::Data<client::Client>,
        state: State,
        request: Json<control::PolicyUpdateRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let request = request.into_inner();
        let local_label = &get_local_service_label(&state, &request.label).await?;
        helper_update(client, state, local_label, request).await 
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
            
            let current = bson::from_bson::<control::PolicyUpdateRequest>(bson::Bson::Document(doc.clone()))
                            .on_err("Bson conversion error")?;
            Ok(HttpResponse::Ok().json(current))
        } else {
            Ok(HttpResponse::NotFound().body(format!("no policy for {}", label)))
        }
    }
    #[get("/query-global")]
    async fn query_global(
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
            
            let current = bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc.clone()))
                            .on_err("Bson conversion error")?;
            Ok(HttpResponse::Ok().json(current))
        } else {
            Ok(HttpResponse::NotFound().body(format!("no policy for {}", label)))
        }
    }
    #[get("/query-onboarding")]
    async fn query_onboarding(
        state: State,
        request: Json<control::PolicyQueryRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let label = &request.label;
        log::info!("querying policy for {}", label);

        let current = control::OnboardingUpdateRequest{
            label: onboarding_policy_label(),
            policy: service::get_onboarding_policy(&state).await?.policy(),
            labels: control::LabelMap::default() 
        };

        Ok(HttpResponse::Ok().json(current))
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
        update_hosts(&client.into_inner(), &state, label, &get_local_service_label(&state, label).await?, &DPPolicies::deny_all()).await?;
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
                update_hosts(&client, &state, &label, &get_local_service_label(&state, &label).await?, &DPPolicies::deny_all()).await?;
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
