pub mod client {
    use bytes::{Bytes, BytesMut};
    use std::io;
    use tokio_io::codec::Decoder;

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
}

pub mod server {
    use bytes::{Bytes, BytesMut};
    use std::io;
    use tokio_io::codec::Decoder;

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
}
