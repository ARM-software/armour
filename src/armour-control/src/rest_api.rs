/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use actix_web::{client, delete, get, post, web, web::Json, HttpResponse};
use armour_api::control::{
    self, 
    global_policy_label, 
    onboarding_policy_label
};
use armour_api::host::PolicyUpdate;
use armour_lang::{
    expressions,
    labels::{Label, Labels}, 
    literals,
    policies::{self, DPPolicies}, 
    literals::OnboardingResult
};
use bson::doc;
use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use super::policy::OnboardingPolicy;
use super::specialize::{compile_egress, compile_ingress};
use super::State;


pub const ARMOUR_DB: &str = "armour";
pub const HOSTS_COL: &str = "hosts";
pub const SERVICES_COL: &str = "services";
pub const POLICIES_COL: &str = "policies";

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
                    literals::Credentials::new(request.credentials.clone()),
                    match request.tmp_dpid { Some(ref x) => x.port(), _ => None},
                    match request.tmp_dpid { Some(ref x) => x.labels.clone(), _=> Labels::default()},
                    match request.tmp_dpid { Some(ref x) => x.ips.clone(), _=> BTreeSet::default()},
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
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        match helper_on_board(&state, request.into_inner()).await? {
            Ok((service_id, ingress_req, egress_req)) =>{
                let merged_request = control::PolicyUpdateRequest{
                    label: service_id.clone(),
                    policy: ingress_req.policy.merge(&egress_req.policy), 
                    labels: ingress_req.labels.into_iter().chain(egress_req.labels.into_iter()).collect()
                };

                policy::save_policy(state.clone(), merged_request).await?;

                Ok(HttpResponse::Ok().json(control::OnboardServiceResponse{
                    service_id: service_id,
                }))
            }, 
            Err(s)=> Ok(internal(s))  
        }
    }

    #[delete("/drop")]
    pub async fn drop(
        state: State,
        request: Json<control::OnboardServiceRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let request = request.into_inner();

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
                    "(allow all)".to_owned()
                } else if policy.policy.is_deny_all() {
                    "(deny all)".to_owned()
                } else {
                    let mut pol : String = "".to_owned();
                    if policy.policy.is_allow_egress() {
                        pol += "(allow egress)";
                    }
                    if policy.policy.is_allow_ingress() {
                        pol += "(allow ingress)";
                    }
                    if policy.policy.is_deny_ingress() {
                        pol += "(deny ingress)";
                    }
                    if policy.policy.is_deny_ingress() {
                        pol += "(deny ingress)";
                    }
                    pol
                };
                s.push_str(format!("{}{}\n", policy.label, pol).as_str())
            }
        }
        Ok(HttpResponse::Ok().body(s))
    }

    pub async fn save_policy(
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

            
            //by default, all onboarded services are concerned
            let selector = &request.selector.unwrap_or(Label::from_str("ServiceID::**").unwrap());
            log::info!("propagating to proxies according to selector: {}", selector);    
            let services = services_full(&state).await?.into_iter().filter(|service|
                service.service_id.has_label(&selector)
            );
            let global_policy = request.policy.clone();
            for service in services {
                let local_label = get_local_service_label(&state, &service.service).await?;

                log::info!("updating policy for {}", service.service);
                let mut local_pol : Option<DPPolicies> = None;
                let arc_state = Arc::new(state.clone()); 
                for function in vec![
                    policies::ALLOW_REST_REQUEST,
                    policies::ALLOW_TCP_CONNECTION,
                ]{ 
                    let tmp_egress_pol = compile_egress(
                        arc_state.clone(), 
                        global_policy.clone(), 
                        function,
                        &service.service_id
                    ).await.map_err(|e| internal(e.to_string()))?;
                    
                    //NB map_or can not be use, it implies one clone
                    local_pol = match local_pol {
                        None => Some(tmp_egress_pol), 
                        Some(pol) => Some(pol.merge(&tmp_egress_pol))
                    };
                }

                for function in vec![
                    policies::ALLOW_REST_RESPONSE,
                    policies::ON_TCP_DISCONNECT,
                ]{ 
                    let tmp_ingress_pol = compile_ingress(
                        arc_state.clone(), 
                        global_policy.clone(), 
                        function,
                        &service.service_id
                    ).await.map_err(|e| internal(e.to_string()))?;

                    local_pol = match local_pol {
                        None => Some(tmp_ingress_pol), 
                        Some(pol) => Some(pol.merge(&tmp_ingress_pol))
                    };
                }
                match local_pol {
                    None =>{ 
                        log::warn!("error no main function in global policy");
                        return Ok(internal("error updating policy of selected services"))
                    }
                    Some(local_pol) =>{
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

                    } 
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
        let mut request = request.into_inner().pack();
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
    #[post("/specialize")]
    pub async fn specialize(
        state: State,
        request: Json<control::SpecializationRequest>,
    ) -> Result<HttpResponse, actix_web::Error> {
        let request = request.into_inner();

        log::info!(r#"Specializing policy"#);


        let mut global_label = Label::concat(&request.host, &request.proxy);
        global_label.prefix("ServiceID".to_string());                    
        let mut global_id = request.cpid.clone();
        global_id = global_id.add_label(&global_label);


        let global_policy = request.policy;
        let arc_state = Arc::new(state.clone()); 
        let mut pol = DPPolicies::default();

        for function in vec![
            policies::ALLOW_REST_REQUEST,
            policies::ALLOW_TCP_CONNECTION,
        ]{ 
            let tmp_egress_pol = compile_egress(
                arc_state.clone(), 
                global_policy.clone(), 
                function,
                &global_id 
            ).await.map_err(|e| internal(e.to_string()))?;
            
            pol = pol.merge(&tmp_egress_pol);
        }

        for function in vec![
            policies::ALLOW_REST_RESPONSE,
            policies::ON_TCP_DISCONNECT,
        ]{ 
            let tmp_ingress_pol = compile_ingress(
                arc_state.clone(), 
                global_policy.clone(), 
                function,
                &global_id 
            ).await.map_err(|e| internal(e.to_string()))?;

            pol = pol.merge(&tmp_ingress_pol);
        }

        let current = control::SpecializationResponse{
            policy: pol,
        };

        Ok(HttpResponse::Ok().json(current))
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
