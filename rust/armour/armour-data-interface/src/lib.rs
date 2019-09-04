/// Communication interface between data plane master and proxy instances

#[macro_use]
extern crate lazy_static;

use actix::prelude::*;
use armour_policy::{
    lang::Program,
    types::{Signature, Typ},
};
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;
use tokio_io::codec::{Decoder, Encoder};

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

pub const ALLOW_REST_REQUEST: &str = "allow_rest_request";
pub const ALLOW_CLIENT_PAYLOAD: &str = "allow_client_payload";
pub const ALLOW_SERVER_PAYLOAD: &str = "allow_server_payload";
pub const ALLOW_TCP_CONNECTION: &str = "allow_tcp_connection";
pub const ON_TCP_DISCONNECT: &str = "on_tcp_disconnect";

lazy_static! {
    pub static ref POLICY_SIG: Vec<(&'static str, Vec<Signature>)> = {
        vec![
            (
                ALLOW_REST_REQUEST,
                vec![
                    Signature::new(
                        vec![Typ::HttpRequest, Typ::ID, Typ::ID, Typ::I64],
                        Typ::Bool,
                    ),
                    Signature::new(vec![Typ::HttpRequest, Typ::ID, Typ::ID], Typ::Bool),
                    Signature::new(vec![Typ::HttpRequest], Typ::Bool),
                    Signature::new(Vec::new(), Typ::Bool),
                ],
            ),
            (
                ALLOW_CLIENT_PAYLOAD,
                vec![
                    Signature::new(vec![Typ::Data, Typ::ID, Typ::ID, Typ::I64], Typ::Bool),
                    Signature::new(vec![Typ::Data, Typ::ID, Typ::ID], Typ::Bool),
                    Signature::new(vec![Typ::Data], Typ::Bool),
                ],
            ),
            (
                ALLOW_SERVER_PAYLOAD,
                vec![
                    Signature::new(vec![Typ::Data, Typ::ID, Typ::ID, Typ::I64], Typ::Bool),
                    Signature::new(vec![Typ::Data, Typ::ID, Typ::ID], Typ::Bool),
                    Signature::new(vec![Typ::Data], Typ::Bool),
                ],
            ),
            (
                ALLOW_TCP_CONNECTION,
                vec![
                    Signature::new(vec![Typ::ID, Typ::ID, Typ::I64], Typ::Bool),
                    Signature::new(vec![Typ::ID, Typ::ID], Typ::Bool),
                ],
            ),
            (
                ON_TCP_DISCONNECT,
                vec![
                    Signature::new(
                        vec![Typ::ID, Typ::ID, Typ::I64, Typ::I64, Typ::I64],
                        Typ::Unit,
                    ),
                    Signature::new(vec![Typ::ID, Typ::ID, Typ::I64], Typ::Unit),
                    Signature::new(vec![Typ::ID, Typ::ID], Typ::Unit),
                ],
            ),
        ]
    };
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub port: u16,
    pub request_streaming: bool,
    pub response_streaming: bool,
}

/// Policy update request messages
#[derive(Serialize, Deserialize, Message, Clone)]
pub enum PolicyRequest {
    AllowAll,
    DenyAll,
    QueryActivePorts,
    Shutdown,
    StopAll,
    Debug(bool),
    Start(ProxyConfig),
    StartTcp(u16),
    Stop(u16),
    UpdateFromData(Program),
    UpdateFromFile(std::path::PathBuf),
    Timeout(u8),
}

/// Messages from proxy instance to master
#[derive(Serialize, Deserialize, Debug, Message)]
pub enum PolicyResponse {
    Started,
    Stopped,
    ShuttingDown,
    UpdatedPolicy,
    RequestFailed,
    ActivePorts {
        http: HashSet<u16>,
        tcp: HashSet<u16>,
    },
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

lazy_static! {
    pub static ref INTERFACE_IPS: HashSet<IpAddr> = {
        let set: HashSet<String> = ["lo", "lo0", "en0", "eth0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
            interfaces
                .into_iter()
                .filter_map(|i| {
                    if set.contains(&i.name) {
                        Some(i.ip())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            HashSet::new()
        }
    };
}

pub fn own_ip(s: &IpAddr) -> bool {
    INTERFACE_IPS.contains(s)
        || match s {
            IpAddr::V4(v4) => v4.is_unspecified() || v4.is_broadcast(),
            IpAddr::V6(v6) => v6.is_unspecified(),
        }
}
