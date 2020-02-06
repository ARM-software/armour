//! Control plane API

use serde::{Deserialize, Serialize};
use url::Url;

type Label = String;
type Credentials = String;
type ArmourProgram = String;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OnboardMasterRequest {
    pub host: Url,
    pub label: Label,
    pub credentials: Credentials, // FIXME change types as needed
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceRequest {
    pub label: Label,   // FIXME
    pub master: String, // FIXME
}

#[derive(Serialize, Deserialize)]
pub struct PolicyUpdateRequest {
    pub service: String, // FIXME
    pub policy: ArmourProgram,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryRequest {
    pub service: String, // FIXME
}
