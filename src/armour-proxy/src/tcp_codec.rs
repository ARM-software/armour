/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

pub mod client {
    use bytes::{Bytes, BytesMut};
    use std::io;
    use tokio_util::codec::Decoder;

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
    use tokio_util::codec::Decoder;

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
