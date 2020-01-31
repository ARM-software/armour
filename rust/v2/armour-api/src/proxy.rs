/// Communication interface between data plane master and proxy instances
use super::{DeserializeDecoder, SerializeEncoder};
use actix::prelude::*;
use armour_lang::lang::Program;
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

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Protocol::HTTP => write!(f, "HTTP"),
            Protocol::TCP => write!(f, "TCP"),
            Protocol::All => write!(f, "TCP+HTTP"),
        }
    }
}

impl FromStr for Protocol {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Protocol::TCP),
            "http" => Ok(Protocol::HTTP),
            "tcp+http" => Ok(Protocol::All),
            _ => Err("failed to parse protocol"),
        }
    }
}

/// Message to proxy instance
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

impl PolicyRequest {
    pub fn valid(&self) -> bool {
        if let PolicyRequest::SetPolicy(_, prog) = self {
            !prog.is_empty()
        } else {
            true
        }
    }
}

/// Transport codec for Master to Proxy instance communication
pub struct PolicyCodec;

impl DeserializeDecoder<PolicyRequest, std::io::Error> for PolicyCodec {}
impl SerializeEncoder<super::master::PolicyResponse, std::io::Error> for PolicyCodec {}

impl Decoder for PolicyCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for PolicyCodec {
    type Item = super::master::PolicyResponse;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}
