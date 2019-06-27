/// Communication interface between data plane master and proxy instances
use actix::prelude::*;
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_io::codec::{Decoder, Encoder};

/// Policy update request messages
#[derive(Serialize, Deserialize, Debug, Message)]
pub enum PolicyRequest {
    UpdateFromFile(std::path::PathBuf),
    UpdateFromData(Vec<u8>),
    AllowAll,
    DenyAll,
}

/// Messages from proxy instance to master
#[derive(Serialize, Deserialize, Debug, Message)]
pub enum PolicyResponse {
    ShuttingDown,
    UpdatedPolicy,
    RquestFailed,
}

/// Transport codec for Master to Proxy instance communication
pub struct PolicyCodec;

impl Decoder for PolicyCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };

        if src.len() >= size + 2 {
            src.split_to(2);
            let buf = src.split_to(size);
            Ok(Some(bincode::deserialize::<PolicyRequest>(&buf).map_err(
                |e| std::io::Error::new(std::io::ErrorKind::Other, e),
            )?))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for PolicyCodec {
    type Item = PolicyResponse;
    type Error = std::io::Error;

    fn encode(&mut self, msg: PolicyResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();

        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);

        Ok(())
    }
}

/// Transport codec for Proxy instance to Master communication
pub struct MasterCodec;

impl Decoder for MasterCodec {
    type Item = PolicyResponse;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };

        if src.len() >= size + 2 {
            src.split_to(2);
            let buf = src.split_to(size);
            Ok(Some(bincode::deserialize::<PolicyResponse>(&buf).map_err(
                |e| std::io::Error::new(std::io::ErrorKind::Other, e),
            )?))
        } else {
            Ok(None)
        }
    }
}


impl Encoder for MasterCodec {
    type Item = PolicyRequest;
    type Error = std::io::Error;

    fn encode(&mut self, msg: PolicyRequest, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();

        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);

        Ok(())
    }
}
