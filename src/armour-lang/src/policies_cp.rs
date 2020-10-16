use super::{
    policies::{FnPolicy, FnPolicies},//TODO should i reuse policies::FnPolicies
    lang::{self, CPProgram},
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


pub const ONBOARDING_SERVICES: &str = "onboarding_policy";


#[derive(Serialize, Deserialize, Clone)]
pub struct OnboardingPolicy {
    //From ProtocolPolicy struct
    name : String,//FIXME usefull ???
    sig : CPSignature,//FIXME only one ??

    //From Policy struct
    pub program: CPProgram,
    //fn_policies: FnPolicies,
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
            name: ONBOARDING_SERVICES.to_string(),
            sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),
            program: CPProgram::default(),
        })
    }
}


//TODO create types : OnboardingData +  OnboardingResult
//FIXME : for now use protocoloPolicy instead of a dedicated OnboardingPolicy
//TODO: only one object Onboarding policiy is need at least for now
lazy_static! {
    static ref ONBOARDING_SERVICES_POLICY: OnboardingPolicy = OnboardingPolicy {
        name: ONBOARDING_SERVICES.to_string(),
        sig: Signature::new(vec![CPTyp::onboardingData()], CPTyp::onboardingResult()),

        program: CPProgram::default(),
    };
}



