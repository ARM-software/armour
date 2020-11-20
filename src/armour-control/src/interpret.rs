/// onboarding policy interpreter
use async_trait::async_trait;
use armour_api::control;
use armour_lang::expressions::{CPExpr, Error, Expr};
use armour_lang::interpret::{CPEnv, TExprInterpreter, TInterpret};
use armour_lang::labels::Label;
use armour_lang::literals::{
    self, Literal,
    CPLiteral, CPID,
    CPFlatLiteral, DPFlatLiteral,
    OnboardingData,
};
use armour_lang::{cplit, cpdplit};
use armour_lang::policies::{GlobalPolicies, DPPolicies};
use armour_lang::types::{CPFlatTyp};
use armour_api::control::{global_policy_label};
use bson::doc;
use futures::future::{BoxFuture};
use super::rest_api::{collection, POLICIES_COL, SERVICES_COL};
use super::specialize;
use super::State;
use std::str::FromStr;
use std::sync::Arc;

fn to_bson<T: ?Sized>(value: &T) -> Result<bson::Bson, self::Error>
where
    T: serde::Serialize,
{
    bson::to_bson(value).on_err("Bson conversion error")
}

pub async fn present(
    col: &mongodb::Collection,
    filter: impl Into<Option<bson::Document>>,
) -> Result<bool, self::Error> {
    use futures::StreamExt;
    Ok(col
        .find(filter, None)
        .await
        .on_err("MongoDB query error")?
        .next()
        .await
        .is_some())
}

//Workaround since we can not do impl CPFLat.. since not in the same crate 
#[async_trait]
pub trait TSLitInterpret {
    fn seval_call0(state: Arc<State>, f: &str) -> Option<CPLiteral>;
    async fn seval_call1(&self, state: Arc<State>, f: &str) -> Result<Option<CPLiteral>, self::Error>;
    async fn seval_call2(&self, state: Arc<State>, f: &str, other: &Self) -> Result<Option<CPLiteral>, self::Error>;
    async fn seval_call3(&self, state: Arc<State>, f: &str, l1: &Self, l2: &Self) -> Result<Option<CPLiteral>, self::Error>;
    async fn seval_call4(&self, state: Arc<State>, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Result<Option<CPLiteral>, self::Error>;
}

#[async_trait]
pub trait TSExprInterpret : Sized{
    async fn seval_call(state: Arc<State>, function: &str, args: Vec<Self>) -> Result<Self, self::Error>;
    fn seval(self, state: Arc<State>, env: CPEnv) -> BoxFuture<'static, Result<Self, self::Error>>;
    async fn sevaluate(self, state: Arc<State>, env: CPEnv) -> Result<Self, self::Error>;
}

trait OnErr<T, E>
where
    Self: Into<Result<T, E>>,
{
    fn on_err(self, b: &str) -> Result<T, self::Error> {
        self.into().map_err(|_| self::Error::new(b.to_string()))
    }
}
impl<T> OnErr<T, bson::de::Error> for bson::de::Result<T> {}
impl<T> OnErr<T, bson::ser::Error> for bson::ser::Result<T> {}
impl<T> OnErr<T, mongodb::error::Error> for mongodb::error::Result<T> {}

pub async fn get_global_pol(state: State) ->  Result<control::CPPolicyUpdateRequest, self::Error> {
    let col = collection(&state, POLICIES_COL);
    if let Ok(Some(doc)) = col
        .find_one(Some(doc! {"label" : to_bson(&global_policy_label())?}), None)
        .await
    {
        match bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc.clone())) {
            Ok(_)=> (),
            Err(e) => println!("{}", e)
        };
        bson::from_bson::<control::CPPolicyUpdateRequest>(bson::Bson::Document(doc))
            .on_err("Bson conversion error")
    } else {
        Ok(control::CPPolicyUpdateRequest{
            label:global_policy_label(),
            policy: GlobalPolicies::default(),
            labels: control::LabelMap::default(),
            selector: None 
        })
    }
}

pub async fn helper_compile_ingress(
    state: Arc<State>, 
    function: &String, 
    id: &CPID
) ->  Result<CPLiteral, self::Error> {
        let global_pol=get_global_pol((*state).clone()).await?;
        
        Ok(literals::Literal::FlatLiteral(CPFlatLiteral::Policy(
            Box::new(literals::Policy::from(
                specialize::compile_ingress(state, global_pol.policy, function, id).await?  
            ))
        )))
}

async fn helper_compile_egress(
    state: Arc<State>, 
    function: &String, 
    id: &CPID
) ->  Result<CPLiteral, self::Error> {
        let global_pol=get_global_pol((*state).clone()).await?;

        Ok(literals::Literal::FlatLiteral(CPFlatLiteral::Policy(
            Box::new(literals::Policy::from(
                specialize::compile_egress(state, global_pol.policy, function, id).await?
            ))
        )))
}

async fn helper_onboarded(state: Arc<State>, obd: &OnboardingData) ->  Result<CPLiteral, self::Error>  {
    let col = collection(&*state, SERVICES_COL);
    let mock_service_id = new_ID(obd);

    if let Ok(Some(doc)) = col
        .find_one(Some(doc! { "service_id.labels" : to_bson(&mock_service_id)? }), None)
        .await
    {
        let request =
            bson::from_bson::<control::POnboardServiceRequest>(bson::Bson::Document(doc))
                .on_err("Bson conversion error")?;
        Ok(cpdplit!(ID(request.service_id.into())).some())
    } else {
        Ok(Literal::none())
    }
}

/// Assumption: (obd.service, obd.host) is unique
#[allow(non_snake_case)]
fn new_ID(obd: &OnboardingData) -> Label {
    let mut service_id = Label::concat(&obd.host(), &obd.service());
    service_id.prefix("ServiceID".to_string());                    
    service_id
}

async fn helper_onboard(state: Arc<State>, id: &CPID) ->  Result<CPLiteral, self::Error>  {
    let host = match id.find_label(&Label::from_str("Host::**")?) {
        Some(l) => l.clone(),
        _ =>  return Err(Error::from(format!("Extracting host from id labels")))
    };

    //local service label
    //let service = match id.find_label(&Label::from_str("Service::**")?) {
    //    Some(l) => l.clone(),
    //    _ =>  return Err(Error::from(format!("Extracting service from id labels")))
    //};

    //global service label
    let service_id = match id.find_label(&Label::from_str("ServiceID::**")?) {
        Some(l) => l.clone(),
        _ =>  return Err(Error::from(format!("Extracting service from id labels")))
    };
    
    let request = control::POnboardServiceRequest {
        service: service_id.clone(),
        service_id: id.clone(),
        host: host.clone()
    };                       
    let col = collection(&*state, SERVICES_COL);
    
    // Check if the service is already there
    if present(&col, doc! { "service_id" : to_bson(&service_id)? }).await? {
        Ok(cpdplit!(Bool(true)))

    } else if let bson::Bson::Document(document) = to_bson(&request)? {
        // Insert into a MongoDB collection
        col.insert_one(document, None) 
            .await
            .on_err("error inserting in MongoDB")?;
        Ok(cpdplit!(Bool(true)))

    } else {
        Ok(cpdplit!(Bool(false)))
    }
}


#[async_trait]
impl TSLitInterpret for CPLiteral {
    fn seval_call0(_state: Arc<State>, f: &str) -> Option<CPLiteral> {
        match f {
            "allow_egress" => {
                Some(literals::Literal::FlatLiteral(CPFlatLiteral::Policy(
                    Box::new(literals::Policy{
                        pol: Box::new(DPPolicies::allow_egress()) 
                    })
                )))
            },
            "allow_ingress" => {
                Some(literals::Literal::FlatLiteral(CPFlatLiteral::Policy(
                    Box::new(literals::Policy{
                        pol: Box::new(DPPolicies::allow_ingress()) 
                    })
                )))
            },
            "deny_egress" => {
                Some(literals::Literal::FlatLiteral(CPFlatLiteral::Policy(
                    Box::new(literals::Policy{
                        pol: Box::new(DPPolicies::deny_egress()) 
                    })
                )))
            },
            "deny_ingress" => {
                Some(literals::Literal::FlatLiteral(CPFlatLiteral::Policy(
                    Box::new(literals::Policy{
                        pol: Box::new(DPPolicies::deny_ingress())
                    })
                )))
            },
            _ => Self::eval_call0(f),
        }
    }
    async fn seval_call1(&self, state: Arc<State>, f: &str) -> Result<Option<CPLiteral>, self::Error> {
        match (f, self) {
            ( "ControlPlane::onboard", cpdplit!(ID(service_id)) ) => { 
                Ok(Some(helper_onboard(state, &service_id.clone().into()).await?))
            },
            ( "ControlPlane::onboarded", cplit!(OnboardingData(obd)) ) => { 
                Ok(Some(helper_onboarded(state, obd).await?))
            }
            ( "ControlPlane::newID", cplit!(OnboardingData(obd)) ) => { 
                let service_id = new_ID(obd);
                let mut service = obd.service();
                service.prefix("Service".to_string());
                let mut host = obd.host();
                host.prefix("Host".to_string());

                let mut id = CPID::default();
                id.port = obd.port();
                let id = id.add_label(&service_id);
                let id = id.add_label(&service);
                let id = id.add_label(&host);

                Ok(Some(cpdplit!(ID(id.into()))))
            }
            _ => Ok(self.eval_call1(f)),
        }
    }
    async fn seval_call2(&self, state: Arc<State>, f: &str, other: &Self) -> Result<Option<CPLiteral>, self::Error> {
        match (f, self, other) {
            ("compile_ingress", cpdplit!(Str(function)), cpdplit!(ID(id))) => {
                Ok(Some(helper_compile_ingress(state, function, &id.clone().into()).await?))
            },
            ("compile_egress", cpdplit!(Str(function)), cpdplit!(ID(id))) =>  {
                Ok(Some(helper_compile_egress(state, function, &id.clone().into()).await?))
            },
            _ => Ok(self.eval_call2(f, other)),
        }
    }

    async fn seval_call3(&self, _state: Arc<State>, f: &str, l1: &Self, l2: &Self) -> Result<Option<CPLiteral>, self::Error> {
        match (f, self, l1, l2) {
            _ => Ok(self.eval_call3(f, l1, l2)),
        }
    }

    #[allow(clippy::many_single_char_names)]
    async fn seval_call4(&self, _state:Arc<State>, f: &str, l1: &Self, l2: &Self, l3: &Self) -> Result<Option<CPLiteral>, self::Error> {
        match (f, self, l1, l2, l3) {
            _ => Ok(self.eval_call4(f, l1, l2, l3)),
        }
    }
}

pub struct CPExprWrapper(CPExpr);

impl CPExprWrapper {
    pub fn new(e: CPExpr) -> Self {
        CPExprWrapper(e)
    }
}

#[async_trait]
impl TExprInterpreter<State, CPFlatTyp, CPFlatLiteral> for CPExprWrapper {
    async fn eval_call(state: Arc<State>, function: &str, args: Vec<CPExpr>) -> Result<CPExpr, self::Error> {
        // builtin function
        match args.as_slice() {
            [] => match Literal::seval_call0(state, function) {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(0): type error")),
            },
            [Expr::LitExpr(l1)] => match l1.seval_call1(state, &function).await? {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(1): type error")),
            },
            [Expr::LitExpr(l1), Expr::LitExpr(l2)] => match l1.seval_call2(state, &function, l2).await? {
                Some(r) => Ok(r.into()),
                None => Err(Error::new("eval, call(2): type error")),
            },
            [Expr::LitExpr(l1), Expr::LitExpr(l2), Expr::LitExpr(l3)] => {
                match l1.seval_call3(state, &function, l2, l3).await? {
                    Some(r) => Ok(r.into()),
                    None => Err(Error::new("eval, call(3): type error")),
                }
            }
            [Expr::LitExpr(l1), Expr::LitExpr(l2), Expr::LitExpr(l3), Expr::LitExpr(l4)] => {
                match l1.seval_call4(state, &function, l2, l3, l4).await? {
                    Some(r) => Ok(r.into()),
                    None => Err(Error::new("eval, call(4): type error")),
                }
            }
            x => Err(Error::from(format!("eval, call: {}: {:?}", function, x))),
        }
    }
}