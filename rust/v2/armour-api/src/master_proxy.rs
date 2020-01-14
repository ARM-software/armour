/// Communication interface between data plane master and proxy instances
use super::{DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::lang::Program;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Serialize, Deserialize, Clone)]
pub enum Protocol {
    All,
    Rest,
    TCP,
}

/// Message from master to proxy instance
#[derive(Serialize, Deserialize, Message, Clone)]
#[rtype("()")]
pub enum PolicyRequest {
    Debug(Protocol, bool),
    SetPolicy(Protocol, Program),
    Shutdown,
    StartHttp(u16),
    StartTcp(u16),
    Status,
    Stop(Protocol),
    Timeout(u8),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Status {
    pub debug: bool,
    pub policy: Program,
    pub port: Option<u16>,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(port) = self.port {
            writeln!(f, "active on port {}", port)?
        } else {
            writeln!(f, "inactive")?
        }
        writeln!(f, "debug is {}", if self.debug { "on" } else { "off" })?;
        write!(f, "policy is: ")?;
        self.policy.print();
        writeln!(f, "{}", self.policy.description())
    }
}

/// Messages from proxy instance to master
#[derive(Serialize, Deserialize, Message)]
#[rtype("()")]
pub enum PolicyResponse {
    Started,
    Stopped,
    ShuttingDown,
    UpdatedPolicy,
    RequestFailed,
    Status { http: Box<Status>, tcp: Box<Status> },
}

/// Transport codec for Master to Proxy instance communication
pub struct PolicyCodec;

/// Transport codec for Proxy instance to Master communication
pub struct MasterCodec;

impl DeserializeDecoder<PolicyRequest, std::io::Error> for PolicyCodec {}
impl SerializeEncoder<PolicyResponse, std::io::Error> for PolicyCodec {}
impl DeserializeDecoder<PolicyResponse, std::io::Error> for MasterCodec {}
impl SerializeEncoder<PolicyRequest, std::io::Error> for MasterCodec {}

impl Decoder for PolicyCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for PolicyCodec {
    type Item = PolicyResponse;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}

impl Decoder for MasterCodec {
    type Item = PolicyResponse;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for MasterCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}
