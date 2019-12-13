/// Communication interface between data plane master and proxy instances
use actix::prelude::*;
use armour_policy::lang::Program;
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_io::codec::{Decoder, Encoder};

#[derive(Serialize, Deserialize, Clone)]
pub enum Protocol {
    All,
    Rest,
    TCP,
}

/// Message from master to proxy instance
#[derive(Serialize, Deserialize, Message, Clone)]
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

trait DeserializeDecoder<T: serde::de::DeserializeOwned, E: std::convert::From<std::io::Error>> {
    fn deserialize_decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, E> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };
        if src.len() >= size + 2 {
            src.split_to(2);
            let buf = src.split_to(size);
            Ok(Some(bincode::deserialize::<T>(&buf).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?))
        } else {
            Ok(None)
        }
    }
}

trait SerializeEncoder<T: serde::Serialize, E: std::convert::From<std::io::Error>> {
    fn serialize_encode(&mut self, msg: T, dst: &mut BytesMut) -> Result<(), E> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();
        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);
        Ok(())
    }
}

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
