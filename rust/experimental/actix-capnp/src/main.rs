use actix::prelude::*;
use bytes::{Bytes, BytesMut};
use futures::Future;
use std::str::FromStr;
use std::{io, net, process};
use tokio_codec::{BytesCodec, FramedRead};
use tokio_io::io::WriteHalf;
use tokio_io::AsyncRead;
use tokio_tcp::TcpStream;

fn main() -> std::io::Result<()> {
    println!("Running actix-capnp test");

    let mut sys = actix::System::new("actix-capnp");

    // let current = sys.block_on(tokio::net::TcpStream::connect(
    //     &"127.0.0.1:8443".parse::<net::SocketAddr>().unwrap(),
    // ));

    // Connect to server
    let addr = net::SocketAddr::from_str("127.0.0.1:8443").expect("failed to parse socket address");
    let result = sys
        .block_on(
            TcpStream::connect(&addr)
                .map_err(|e| {
                    println!("filed to connect to server: {}", e);
                    process::exit(1)
                })
                .and_then(|stream| {
                    let addr = Client::create(|ctx| {
                        let (r, w) = stream.split();
                        ctx.add_stream(FramedRead::new(r, BytesCodec::new()));
                        Client {
                            message: Bytes::new(),
                            framed: actix::io::FramedWrite::new(w, BytesCodec::new(), ctx),
                        }
                    });
                    addr.do_send(ClientCommand::SendMessage);
                    let res = addr.send(ClientCommand::GetReply);
                    res.map(|res| {
                        println!("got: {:?}", res);
                        res
                    })
                    .map_err(|_| ())
                }),
        )
        .unwrap();

    println!("result: {:?}", result);

    sys.run()
}

enum ClientCommand {
    SendMessage,
    GetReply,
}

impl Message for ClientCommand {
    type Result = Option<Bytes>;
}

impl Handler<ClientCommand> for Client {
    type Result = Option<Bytes>;

    fn handle(&mut self, msg: ClientCommand, _: &mut Context<Self>) -> Self::Result {
        match msg {
            ClientCommand::SendMessage => {
                println!("sending");
                self.framed.write(bytes::Bytes::from_static(b"hello"));
                None
            }
            ClientCommand::GetReply => {
                println!("getting reply");
                Some(self.message.clone())
            }
        }
    }
}

struct Client {
    message: Bytes,
    framed: actix::io::FramedWrite<WriteHalf<TcpStream>, BytesCodec>,
}

impl Actor for Client {
    type Context = Context<Self>;

    fn stopping(&mut self, _: &mut Context<Self>) -> Running {
        println!("disconnected");

        // Stop application on disconnect
        System::current().stop();

        Running::Stop
    }
}

impl actix::io::WriteHandler<io::Error> for Client {}

/// Server communication
impl StreamHandler<BytesMut, io::Error> for Client {
    fn handle(&mut self, msg: BytesMut, _: &mut Context<Self>) {
        println!("received: {:?}", msg)
    }
}