use actix::*;
use futures::{Stream, Future};
use actix::{Actor, Context, System, Handler};
use actix::AsyncContext;
use tokio_uds;
use tokio_codec::{FramedRead};
use tokio_io::{AsyncRead};

#[macro_use]
extern crate log;
extern crate env_logger;

use docker_lib as lib;

#[derive(Default)]
pub struct DockerCPStub {
    pub socket: String, 
}

impl Actor for DockerCPStub {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Context<Self>) {
        info!("Docker control plane stub server");
    }

    fn stopped(&mut self, _: &mut Context<Self>) {
        info!("Docker control plane stub terminated");
    }
}

impl Handler<lib::UdsConnect> for DockerCPStub {
    type Result = ();
    
    fn handle(&mut self, msg: lib::UdsConnect, _: &mut Context<Self>) {
        println!("Connecting to the data plane");
        lib::DockerControlPlane::create(move |ctx| {
            let (r, w) = msg.0.split();
            lib::DockerControlPlane::add_stream(FramedRead::new(r, lib::DockerEventCodec), ctx);
            lib::DockerControlPlane {
                uds_framed: actix::io::FramedWrite::new(w, lib::DockerEventCodec, ctx),
            }
        });
    }
}

fn main() {
    env_logger::init();

    let system = System::new("Docker-control-plane-stub");

    let ars : Vec<String> = std::env::args().collect();
    let socket = ars[1].clone();

    info!("Address is {}", socket.to_string());
    
    let listener = tokio_uds::UnixListener::bind(socket.to_string()).unwrap();
    
    let _server = DockerCPStub::create(move |ctx| {
        ctx.add_message_stream(
            listener
                .incoming()
                .map_err(|_| ())
                .map(|st| lib::UdsConnect(st)),
        );
        DockerCPStub{
            socket: socket,
        }
    });

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
