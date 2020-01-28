/// API of data plane master
use super::{DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::lang::Program;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

/// Control plane messages to master
#[derive(Serialize, Deserialize, Debug)]
pub struct PolicyUpdate {
    pub label: String,
    pub policy: String,
}

/// Proxy messages to master
#[derive(Serialize, Deserialize, Message)]
#[rtype("()")]
pub enum PolicyResponse {
    Connect(u32),
    Started,
    Stopped,
    ShuttingDown,
    UpdatedPolicy,
    RequestFailed,
    Status { http: Box<Status>, tcp: Box<Status> },
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

/// Transport codec for Proxy instance to Master communication
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
