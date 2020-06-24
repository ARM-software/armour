//! Control plane API

use armour_lang::labels::{Label, Labels};
use armour_lang::policies::Policies;
use serde::{Deserialize, Serialize};

pub const CONTROL_PLANE: &str = "http://localhost:8088";
pub const TCP_PORT: u16 = 8088;

type Credentials = String;
// map from domains to labels
pub type LabelMap = std::collections::BTreeMap<String, Labels>;

#[derive(Clone, Serialize, Deserialize)]
pub struct OnboardHostRequest {
    pub host: url::Url,
    pub label: Label,
    pub credentials: Credentials, // FIXME change types as needed
}

#[derive(Serialize, Deserialize)]
pub struct OnboardServiceRequest {
    pub service: Label,
    pub host: Label,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyUpdateRequest {
    pub label: Label,
    pub policy: Policies,
    pub labels: LabelMap,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryRequest {
    pub label: Label,
}

#[derive(Serialize, Deserialize)]
pub struct PolicyQueryResponse {
    pub policy: Policies,
    pub labels: LabelMap,
}
