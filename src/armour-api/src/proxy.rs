/// Communication interface between data plane master and proxy instances
use super::{master, DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::{labels, lang};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Protocol {
    All,
    HTTP,
    TCP,
}

impl Protocol {
    pub fn interface(&self) -> &lang::Interface {
        match self {
            Protocol::HTTP => &*lang::HTTP_POLICY,
            Protocol::TCP => &*lang::TCP_POLICY,
            Protocol::All => &*lang::TCP_HTTP_POLICY,
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Protocol::HTTP => write!(f, "http"),
            Protocol::TCP => write!(f, "tcp"),
            Protocol::All => write!(f, "all"),
        }
    }
}

impl FromStr for Protocol {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tcp" => Ok(Protocol::TCP),
            "http" => Ok(Protocol::HTTP),
            "all" => Ok(Protocol::All),
            _ => Err(format!("failed to parse protocol: {}", s)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum Policy {
    AllowAll(Protocol),
    DenyAll(Protocol),
    Program(lang::Program),
}

impl std::convert::TryFrom<&master::Policy> for Policy {
    type Error = String;
    fn try_from(policy: &master::Policy) -> Result<Self, Self::Error> {
        match policy {
            master::Policy::AllowAll(p) => Ok(Policy::AllowAll(p.clone())),
            master::Policy::DenyAll(p) => Ok(Policy::DenyAll(p.clone())),
            master::Policy::Bincode(s) => {
                let prog = lang::Program::from_bincode_raw(s.as_bytes())
                    .map_err(|err| format!("failed to parse policy bincode:\n{}", err))?;
                let _protocol = prog.protocol().parse::<Protocol>()?;
                Ok(Policy::Program(prog))
            }
        }
    }
}

impl fmt::Display for Policy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Policy::AllowAll(protocol) => write!(f, "allow all {}", protocol),
            Policy::DenyAll(protocol) => write!(f, "deny all {}", protocol),
            Policy::Program(prog) => {
                write!(f, "{} policy {}", prog.protocol(), prog.blake3_string())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum LabelOp {
    AddIp(Vec<(std::net::Ipv4Addr, labels::Labels)>),
    AddUri(Vec<(String, labels::Labels)>),
    RemoveIp(std::net::Ipv4Addr, Option<labels::Label>),
    RemoveUri(String, Option<labels::Label>),
    Clear,
}

/// Message to proxy instance
#[derive(Serialize, Deserialize, Message, Clone)]
#[rtype("()")]
pub enum PolicyRequest {
    Label(LabelOp),
    SetPolicy(Policy),
    Shutdown,
    StartHttp(u16),
    StartTcp(u16),
    Status,
    Stop(Protocol),
    Timeout(u8),
}

impl PolicyRequest {
    pub fn valid(&self) -> bool {
        if let PolicyRequest::SetPolicy(Policy::Program(prog)) = self {
            !prog.is_empty()
        } else {
            true
        }
    }
}

/// Transport codec for Master to Proxy instance communication
pub struct PolicyCodec;

impl DeserializeDecoder<PolicyRequest, std::io::Error> for PolicyCodec {}
impl SerializeEncoder<master::PolicyResponse, std::io::Error> for PolicyCodec {}

impl Decoder for PolicyCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for PolicyCodec {
    type Item = master::PolicyResponse;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}
