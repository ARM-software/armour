//! Establishes APIs for Armour components

use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut};

pub mod control;
pub mod host;
pub mod proxy;

trait DeserializeDecoder<T: serde::de::DeserializeOwned, E: std::convert::From<std::io::Error>> {
    fn deserialize_decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, E> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };
        if src.len() >= size + 2 {
            src.advance(2);
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
        dst.put_u16(msg_ref.len() as u16);
        dst.put(msg_ref);
        Ok(())
    }
}
