//! Data plane `host` API

use crate::proxy::HttpConfig;
use crate::{DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::{
    labels::{Label, Labels},
    literals::{DPID},
    policies,
};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio_util::codec::{Decoder, Encoder};

pub const DATA_PLANE_HOST: &str = "https://localhost:8090";
pub const TCP_PORT: u16 = 8090;
pub const UDS_SOCKET: &str = "armour";

/// Request policy update
#[derive(Serialize, Deserialize)]
pub struct PolicyUpdate {
    pub label: Label,
    pub policy: policies::DPPolicies,
}

/// Query current policy status
#[derive(Serialize, Deserialize)]
pub struct PolicyQuery {
    pub label: Label,
    pub potocol: policies::DPProtocol,
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

/// Message from `proxy` instance to `host`
#[derive(Serialize, Deserialize, Message)]
#[rtype("()")]
pub enum PolicyResponse {
    Connect(u32, Option<DPID>, Label, String, String), // (PID, name, http hash, tcp hash)
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
    UpdatedPolicy(policies::DPProtocol, String), // hash of new policy
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Status {
    pub policy: policies::DPPolicy,
    pub port: Option<u16>,
    pub ingress: Option<std::net::SocketAddrV4>,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(port) = self.port {
            writeln!(f, "active on port {}", port)?
        } else {
            writeln!(f, "inactive")?
        }
        if let Some(ingress) = self.ingress {
            writeln!(f, "ingress for: {}", ingress)?
        }
        write!(f, "policy is: {}", self.policy)
    }
}

pub type Proxies = Vec<Proxy>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Proxy {
    pub label: Label,
    port: Option<u16>,
    pub timeout: Option<u8>,
    #[serde(default)]
    pub debug: bool,
    ingress: Option<String>,
}

impl Proxy {
    pub fn port(&self, mut p: u16) -> u16 {
        self.port.unwrap_or_else(|| {
            p += 1;
            p
        })
    }
    pub fn set_ingress(
        &mut self,
        hosts: &BTreeMap<String, std::net::Ipv4Addr>,
    ) -> Result<(), String> {
        if self.ingress().is_none() {
            if let Some(ingress) = self.ingress.as_mut() {
                match ingress.split(':').collect::<Vec<&str>>().as_slice() {
                    [host, port] => match (hosts.get(&(*host).to_string()), port.parse::<u16>()) {
                        (Some(host_ip), Ok(port)) => {
                            *ingress = std::net::SocketAddrV4::new(*host_ip, port).to_string();
                        }
                        _ => return Err(format!("failed to set ingress: {}", ingress)),
                    },
                    _ => return Err(format!("failed to set ingress: {}", ingress)),
                }
            }
        };
        Ok(())
    }
    pub fn ingress(&self) -> Option<std::net::SocketAddrV4> {
        self.ingress
            .as_ref()
            .map(|s| s.parse::<std::net::SocketAddrV4>().ok())
            .flatten()
    }
    pub fn config(&self, p: u16) -> HttpConfig {
        let port = self.port(p);
        if let Some(ingress) = self.ingress() {
            HttpConfig::Ingress(port, ingress)
        } else {
            HttpConfig::Port(port)
        }
    }
}

impl From<Label> for Proxy {
    fn from(label: Label) -> Self {
        Proxy {
            label,
            port: None,
            timeout: None,
            debug: false,
            ingress: None,
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

/// Tokio utils codec for `proxy` instance to `host` communication
pub struct HostCodec;

impl DeserializeDecoder<PolicyResponse, std::io::Error> for HostCodec {}
impl SerializeEncoder<super::proxy::PolicyRequest, std::io::Error> for HostCodec {}

impl Decoder for HostCodec {
    type Item = PolicyResponse;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder<super::proxy::PolicyRequest> for HostCodec {
    type Error = std::io::Error;
    fn encode(
        &mut self,
        msg: super::proxy::PolicyRequest,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}
