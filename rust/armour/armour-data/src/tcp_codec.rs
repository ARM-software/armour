fn out_of_memory(needed: usize) -> bool {
    let mut v: Vec<u8> = Vec::with_capacity(0);
    v.try_reserve(needed).is_err()
}

pub mod client {
    use bytes::{Bytes, BytesMut};
    use std::io;
    use tokio_io::codec::{Decoder, Encoder};

    /// A simple `Codec` implementation that just ships bytes around.
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
    pub struct ClientCodec;

    pub struct ClientBytes(pub Bytes);

    impl Decoder for ClientCodec {
        type Item = ClientBytes;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<ClientBytes>, io::Error> {
            Ok(if !buf.is_empty() {
                Some(ClientBytes(buf.split_to(buf.len()).freeze()))
            } else {
                None
            })
        }
    }

    impl Encoder for ClientCodec {
        type Item = Bytes;
        type Error = io::Error;

        fn encode(&mut self, data: Bytes, buf: &mut BytesMut) -> Result<(), io::Error> {
            if super::out_of_memory(buf.len()) {
                log::warn!("out of memory");
                return Err(io::Error::new(io::ErrorKind::Other, "out of memory"));
            }
            buf.extend_from_slice(&data);
            Ok(())
        }
    }
}

pub mod server {
    use bytes::{Bytes, BytesMut};
    use std::io;
    use tokio_io::codec::{Decoder, Encoder};

    /// A simple `Codec` implementation that just ships bytes around.
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
    pub struct ServerCodec;

    pub struct ServerBytes(pub Bytes);

    impl Decoder for ServerCodec {
        type Item = ServerBytes;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<ServerBytes>, io::Error> {
            Ok(if !buf.is_empty() {
                Some(ServerBytes(buf.split_to(buf.len()).freeze()))
            } else {
                None
            })
        }
    }

    impl Encoder for ServerCodec {
        type Item = Bytes;
        type Error = io::Error;

        fn encode(&mut self, data: Bytes, buf: &mut BytesMut) -> Result<(), io::Error> {
            if super::out_of_memory(buf.len()) {
                log::warn!("out of memory");
                return Err(io::Error::new(io::ErrorKind::Other, "out of memory"));
            }
            buf.extend_from_slice(&data);
            Ok(())
        }
    }
}
