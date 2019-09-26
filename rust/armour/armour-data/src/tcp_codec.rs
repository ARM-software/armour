pub mod client {
    use bytes::{BufMut, Bytes, BytesMut};
    use std::io;
    use tokio_io::codec::{Decoder, Encoder};

    /// A simple `Codec` implementation that just ships bytes around.
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
    pub struct ClientCodec;

    pub struct ClientBytes(pub BytesMut);

    impl ClientCodec {
        /// Creates a new `ClientCodec` for shipping around raw bytes.
        pub fn new() -> ClientCodec {
            ClientCodec
        }
    }

    impl Decoder for ClientCodec {
        type Item = ClientBytes;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<ClientBytes>, io::Error> {
            if !buf.is_empty() {
                let len = buf.len();
                Ok(Some(ClientBytes(buf.split_to(len))))
            } else {
                Ok(None)
            }
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

    pub struct ServerBytes(pub BytesMut);

    impl ServerCodec {
        /// Creates a new `ServerCodec` for shipping around raw bytes.
        pub fn new() -> ServerCodec {
            ServerCodec
        }
    }

    impl Decoder for ServerCodec {
        type Item = ServerBytes;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<ServerBytes>, io::Error> {
            if !buf.is_empty() {
                let len = buf.len();
                Ok(Some(ServerBytes(buf.split_to(len))))
            } else {
                Ok(None)
            }
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
