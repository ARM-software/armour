/// Communication interface between data plane host and proxy instances
use super::{host, DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::{labels, policies};
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Serialize, Deserialize, Clone)]
pub enum LabelOp {
    AddIp(Vec<(std::net::Ipv4Addr, labels::Labels)>),
    AddUri(Vec<(String, labels::Labels)>),
    RemoveIp(std::net::Ipv4Addr, Option<labels::Label>),
    RemoveUri(String, Option<labels::Label>),
    Clear,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HttpConfig {
    Port(u16),
    Ingress(u16, std::net::SocketAddrV4),
}

impl HttpConfig {
    pub fn port(&self) -> u16 {
        match self {
            HttpConfig::Port(p) => *p,
            HttpConfig::Ingress(p, _) => *p,
        }
    }
    pub fn ingress(&self) -> Option<std::net::SocketAddrV4> {
        match self {
            HttpConfig::Port(_p) => None,
            HttpConfig::Ingress(_p, socket) => Some(*socket),
        }
    }
}

/// Message to proxy instance
#[derive(Serialize, Deserialize, Message, Clone)]
#[rtype("()")]
pub enum PolicyRequest {
    Label(LabelOp),
    SetPolicy(policies::Policies),
    Shutdown,
    StartHttp(HttpConfig),
    StartTcp(u16),
    Status,
    Stop(policies::Protocol),
    Timeout(u8),
}

/// Transport codec for Host to Proxy instance communication
pub struct PolicyCodec;

impl DeserializeDecoder<PolicyRequest, std::io::Error> for PolicyCodec {}
impl SerializeEncoder<host::PolicyResponse, std::io::Error> for PolicyCodec {}

impl Decoder for PolicyCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder<host::PolicyResponse> for PolicyCodec {
    type Error = std::io::Error;
    fn encode(&mut self, msg: host::PolicyResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}
