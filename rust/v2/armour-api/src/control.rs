//! Control plane API

use armour_lang::labels::Label;
use serde::{Deserialize, Serialize};
use url::Url;

type Credentials = String;
type ArmourProgram = String;

#[derive(Clone, Serialize, Deserialize)]
pub struct OnboardMasterRequest {
    pub host: Url,
    pub master: Label,
    pub credentials: Credentials, // FIXME change types as needed
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceRequest {
    pub service: Label, // FIXME
    pub master: Label,  // FIXME
}

#[derive(Serialize, Deserialize)]
pub struct PolicyUpdateRequest {
    pub service: Label, // FIXME
    pub policy: ArmourProgram,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryRequest {
    pub service: Label, // FIXME
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryResponse {
    pub policy: ArmourProgram, // FIXME
}
