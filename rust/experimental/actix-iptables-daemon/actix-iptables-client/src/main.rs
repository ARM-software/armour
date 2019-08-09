use actix::{Actor, System};
use actix::prelude::*;
use tokio_uds;
use tokio_io::{AsyncRead};

#[macro_use]
extern crate log;
extern crate env_logger;

use iptables_lib as lib;

fn main() {

    env_logger::init();

    let system = System::new("iptables-client");

    let ars : Vec<String> = std::env::args().collect();
    let socket = ars[1].clone();

    info!("Address is {}", socket.to_string());

    Arbiter::spawn(
        tokio_uds::UnixStream::connect(socket.to_string())
            .and_then(|stream| {
                let addr = lib::IptablesClientActor::create(|ctx| {
                    let (_r, w) = stream.split();
                    lib::IptablesClientActor {
                        uds_framed: actix::io::FramedWrite::new(
                            w,
                            lib::IptablesCodec,
                            ctx,
                        ),
                    }
                });
                addr.do_send(
                    lib::IptablesCommands::NewChain{
                        table: "nat".to_string(),
                        chain: "mychain".to_string()}
                );
                addr.do_send(
                    lib::IptablesCommands::DeleteChain {
                        table: "nat".to_string(),
                        chain: "mychain".to_string()}
                );
                futures::future::ok(())
            })
            .map_err(|_| ())
    );

    let ctrl_c = tokio_signal::ctrl_c().flatten_stream();
    let handle_shutdown = ctrl_c
        .for_each(|()| {
            println!("Ctrl-C received, shutting down");
            System::current().stop();
            Ok(())
        })
        .map_err(|_| ());
    actix::spawn(handle_shutdown);
    
    let _ = system.run();
}
