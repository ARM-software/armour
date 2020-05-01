//! Data plane `master` API

use super::{proxy, DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::{
    labels::{Label, Labels},
    lang::Program,
};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio_util::codec::{Decoder, Encoder};

/// Policy type during `control` plane to `master` communication
///
/// There are three main possibilities:
/// 1. Allow all (HTTP, TCP or both)
/// 2. Deny all (HTTP, TCP or both)
/// 3. A policy program, encoded as a [String](https://doc.rust-lang.org/std/string/struct.String.html) using Bincode (serde), Gzip and Base64
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Policy {
    AllowAll(proxy::Protocol),
    DenyAll(proxy::Protocol),
    Bincode(String),
}

/// Request policy update
///
/// Consists of a label, which should be of the form `<master>::<proxy>`, and a [Policy](enum.Policy.html) value
#[derive(Serialize, Deserialize, Debug)]
pub struct PolicyUpdate {
    pub label: Label,
    pub policy: Policy,
}

/// Query current policy status
#[derive(Serialize, Deserialize, Debug)]
pub struct PolicyQuery {
    pub label: Label,
    pub potocol: proxy::Protocol,
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
    UpdatedPolicy(proxy::Protocol, String), // hash of new policy
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Status {
    pub policy: Program,
    pub port: Option<u16>,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.policy.print();
        if let Some(port) = self.port {
            writeln!(f, "active on port {}", port)?
        } else {
            writeln!(f, "inactive")?
        }
        write!(f, "policy is: {}", self.policy.description())
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OnboardInformation {
    pub proxies: Proxies,
    pub labels: Vec<(std::net::Ipv4Addr, Labels)>,
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
