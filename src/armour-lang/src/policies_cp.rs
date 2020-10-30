use super::{
    expressions,
    policies::{GlobalPolicy, Policy,ProtocolPolicy, FnPolicy, FnPolicies},//TODO should i reuse policies::FnPolicies
    lang::{self, CPProgram, CPPreProgram},
    literals::CPFlatLiteral,
    types_cp::{CPFlatTyp, CPTyp, CPSignature},
    types::{Signature, Typ},
    parser
};

use lazy_static::lazy_static;
use serde::{
    de::{Deserializer, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Serialize,
};
use std::collections::BTreeMap;


pub const ONBOARDING_SERVICES: &str = "onboarding_policy";

pub type OnboardingPolicy = GlobalPolicy;
//#[derive(Serialize, Deserialize, Clone, Debug)]
//pub struct OnboardingPolicy {
//    //From ProtocolPolicy struct
//    pub name : String,//FIXME usefull ???
//    sig : CPSignature,//FIXME only one ??
//
//    //From Policy struct
//    program: CPProgram,
//    //fn_policies: FnPolicies,
//}

impl OnboardingPolicy {
    pub fn program<'a>(&'a self) -> &'a CPProgram {
        &self.program
    }
    fn inner_from(pre_prog: lang::CPPreProgram) -> Result<Self, expressions::Error> {
        Ok(OnboardingPolicy {
            //name: ONBOARDING_SERVICES.to_string(),
            //sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
            fn_policies: FnPolicies::default(),
            program: pre_prog.program(&vec![ONBOARDING_SERVICES.to_string()][..]),
        })
    }

    pub fn from_buf(buf: &str) -> Result<Self, expressions::Error> {
        Self::inner_from(lang::PreProgram::from_buf(buf)?)
    }
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, expressions::Error> {
        Self::inner_from(lang::PreProgram::from_file(path)?)
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub enum ObPolicy {
    None, //Onboard no services
    Custom(OnboardingPolicy) //Use cuserd defined policy
}

impl ObPolicy {
    pub fn onboard_none() -> Self {
        Self::None
    }
    pub fn onboard_from(p: CPProgram) -> Self {
        Self::Custom(OnboardingPolicy {
            //name: ONBOARDING_SERVICES.to_string(),
            //sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
            fn_policies: FnPolicies::default(),
            program: p,
        })
    }
    pub fn from_buf(buf: &str) -> Result<Self, expressions::Error> {
        Ok(Self::Custom(OnboardingPolicy::from_buf(buf)?))
    }
}


//TODO create types : OnboardingData +  OnboardingResult
//FIXME : for now use protocoloPolicy instead of a dedicated OnboardingPolicy
//TODO: only one object Onboarding policiy is need at least for now
lazy_static! {
    static ref ONBOARDING_SERVICES_POLICY: OnboardingPolicy = OnboardingPolicy {
        //name: ONBOARDING_SERVICES.to_string(),
        //sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
        fn_policies: FnPolicies::default(),
        program: CPProgram::default(),
    };
}



