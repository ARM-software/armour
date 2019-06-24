use actix::prelude::*;
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_io::codec::{Decoder, Encoder};

#[derive(Serialize, Deserialize, Debug)]
pub enum ArmourPolicyRequest {
    UpdateFromFile(std::path::PathBuf),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ArmourPolicyResponse {
    Ack,
}

impl Message for ArmourPolicyRequest {
    type Result = std::io::Result<()>;
}

impl Message for ArmourPolicyResponse {
    type Result = std::io::Result<()>;
}

/// Codec for Master -> Data transport
pub struct ArmourDataCodec;

impl Decoder for ArmourDataCodec {
    type Item = ArmourPolicyRequest;
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
            Ok(Some(
                bincode::deserialize::<ArmourPolicyRequest>(&buf)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?,
            ))
        } else {
            Ok(None)
        }
    }
}

impl Encoder for ArmourDataCodec {
    type Item = ArmourPolicyResponse;
    type Error = std::io::Error;

    fn encode(&mut self, msg: ArmourPolicyResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();

        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);

        Ok(())
    }
}

/// Codec for Data -> Master transport
pub struct MasterArmourDataCodec;

impl Decoder for MasterArmourDataCodec {
    type Item = ArmourPolicyResponse;
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
            Ok(Some(
                bincode::deserialize::<ArmourPolicyResponse>(&buf)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?,
            ))
        } else {
            Ok(None)
        }
    }
}


impl Encoder for MasterArmourDataCodec {
    type Item = ArmourPolicyRequest;
    type Error = std::io::Error;

    fn encode(&mut self, msg: ArmourPolicyRequest, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();

        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);

        Ok(())
    }
}
