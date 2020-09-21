use super::{
    policies::{FnPolicy, FnPolicies},//TODO should i reuse policies::FnPolicies
    lang::{Program},
    types_cp::{CPTyp},
    types::{Signature, Typ},
};

use lazy_static::lazy_static;
use serde::{
    de::{Deserializer, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Serialize,
};


pub const ONBOARDING_SERVICES: &str = "onboardingPolicy";
//N.B: pub const ONBOARDING_HOSTS: &str = "onboardingHostPolicy";

#[derive(Serialize, Deserialize, Clone)]
pub struct OnboardingPolicy {
    //From ProtocolPolicy struct
    name : String,//FIXME usefull ???
    sig : Signature<CPTyp>,//FIXME only one ??

    //From Policy struct
    pub program: Program,//TODO should i define a CPProgram
    fn_policies: FnPolicies,
}

//TODO create types : OnboardingData +  OnboardingResult
//FIXME : for now use protocoloPolicy instead of a dedicated OnboardingPolicy
//TODO: only one object Onboarding policiy is need at least for now
lazy_static! {
    static ref ONBOARDING_SERVICES_POLICY: OnboardingPolicy = OnboardingPolicy {
        name: ONBOARDING_SERVICES.to_string(),
        sig: Signature::new(vec![CPTyp::OnboardingData], CPTyp::OnboardingResult),

        program: Program::default(),
        fn_policies: FnPolicies::default()
    };
}



