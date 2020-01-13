// use url_serde::SerdeUrl;
// use url::Url;
use armour_policy::lang::Program;
use serde::{Serialize, Deserialize};

type Credentials = String;
type LocalID = String;
type GlobalID = String;
type Label = String;
type UID = String;
type Version = i32;

#[derive(Serialize, Deserialize)]
pub struct MasterMetadata {
        pub host_url: String,
        pub local_id: LocalID,
        pub global_id: GlobalID,
        pub credentials: Credentials,
        pub uid: UID,
        pub labels: Vec<Label>,
        pub services: Vec<UID>, // FIXME: This type might need to be refined
}

#[derive(Serialize, Deserialize)]
pub struct ServiceMetadata {
        pub local_id: LocalID,
        pub global_id: GlobalID,
        pub credentials: Credentials,
        pub uid: UID,
        pub labels: Vec<Label>,
        pub master_id: Option<UID>,      // FIXME: This type might need to be refined
        pub policy: Option<Program>,
}

#[derive(Serialize, Deserialize)]
pub struct Policy {
        pub labels: Vec<Label>, // Labels to which the policy applies (for fast query)
        pub policy: Option<Program>,
        pub version: Option<Version>,
}
