//! Data plane `master` API

use super::{DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::{
    labels::{Label, Labels},
    policies,
};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio_util::codec::{Decoder, Encoder};

/// Request policy update
///
/// Consists of a label, which should be of the form `<master>::<proxy>`
#[derive(Serialize, Deserialize)]
pub struct PolicyUpdate {
    pub label: Label,
    pub policy: policies::Policies,
}

/// Query current policy status
#[derive(Serialize, Deserialize)]
pub struct PolicyQuery {
    pub label: Label,
    pub potocol: policies::Protocol,
}

/// Current policy status
///
/// Consists of proxy `name` and (blake3) hashes of current HTTP and TCP policies
#[derive(Serialize, Deserialize, Debug)]
pub struct PolicyStatus {
    pub label: Label,
    pub http: String, // hash
    pub tcp: String,  // hash
}

/// Message from `proxy` instance to `master`
#[derive(Serialize, Deserialize, Message)]
#[rtype("()")]
pub enum PolicyResponse {
    Connect(u32, Label, String, String), // (PID, name, http hash, tcp hash)
    RequestFailed,
    ShuttingDown,
    Started,
    Status {
        label: Label,
        labels: BTreeMap<String, Labels>,
        http: Box<Status>,
        tcp: Box<Status>,
    },
    Stopped,
    UpdatedPolicy(policies::Protocol, String), // hash of new policy
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Status {
    pub policy: policies::Policy,
    pub port: Option<u16>,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(port) = self.port {
            writeln!(f, "active on port {}", port)?
        } else {
            writeln!(f, "inactive")?
        }
        write!(f, "policy is: {}", self.policy)
    }
}

pub type Proxies = Vec<Proxy>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Proxy {
    pub label: Label,
    pub port: Option<u16>,
    pub timeout: Option<u8>,
    #[serde(default)]
    pub debug: bool,
}

impl From<Label> for Proxy {
    fn from(label: Label) -> Self {
        Proxy {
            label,
            port: None,
            timeout: None,
            debug: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnboardInformation {
    pub proxies: Proxies,
    pub labels: Vec<(std::net::Ipv4Addr, Labels)>,
}

impl OnboardInformation {
    pub fn top_port(&self) -> u16 {
        self.proxies
            .iter()
            .filter_map(|proxy| proxy.port)
            .max()
            .unwrap_or(5999)
    }
}

/// Tokio utils codec for `proxy` instance to `master` communication
pub struct MasterCodec;

impl DeserializeDecoder<PolicyResponse, std::io::Error> for MasterCodec {}
impl SerializeEncoder<super::proxy::PolicyRequest, std::io::Error> for MasterCodec {}

impl Decoder for MasterCodec {
    type Item = PolicyResponse;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for MasterCodec {
    type Item = super::proxy::PolicyRequest;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}
