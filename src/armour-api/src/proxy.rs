/// Communication interface between data plane master and proxy instances
use super::{master, DeserializeDecoder, SerializeEncoder};
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

/// Message to proxy instance
#[derive(Serialize, Deserialize, Message, Clone)]
#[rtype("()")]
pub enum PolicyRequest {
    Label(LabelOp),
    SetPolicy(policies::Policies),
    Shutdown,
    StartHttp(u16),
    StartTcp(u16),
    Status,
    Stop(policies::Protocol),
    Timeout(u8),
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