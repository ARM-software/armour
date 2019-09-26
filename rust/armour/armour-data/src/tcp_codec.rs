pub mod client {
    use bytes::{BufMut, Bytes, BytesMut};
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
            buf.reserve(data.len());
            buf.put(data);
            Ok(())
        }
    }
}

pub mod server {
    use bytes::{BufMut, Bytes, BytesMut};
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
            buf.reserve(data.len());
            buf.put(data);
            Ok(())
        }
    }
}
