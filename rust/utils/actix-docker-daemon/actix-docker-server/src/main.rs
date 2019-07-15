use actix::*;
use futures::{Stream, Future};
use actix::{Actor, Context, System, Handler};
use actix::AsyncContext;
use tokio_uds;
use tokio_codec::{FramedRead};
use tokio_io::{AsyncRead};
use shiplift;

#[macro_use]
extern crate log;
extern crate env_logger;

use docker_lib as lib;

#[derive(Default)]
pub struct DockerActorServer {
    pub socket: String, 
}

impl Actor for DockerActorServer {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Context<Self>) {
        info!("Docker server started");
    }

    fn stopped(&mut self, _: &mut Context<Self>) {
        info!("Docker server terminated");
    }
}

impl Handler<lib::UdsConnect> for DockerActorServer {
    type Result = ();
    
    fn handle(&mut self, msg: lib::UdsConnect, _: &mut Context<Self>) {
        lib::DockerActor::create(move |ctx| {
            let (r, w) = msg.0.split();
            lib::DockerActor::add_stream(FramedRead::new(r, lib::DockerCodec), ctx);
            lib::DockerActor {
                uds_framed: actix::io::FramedWrite::new(w, lib::DockerCodec, ctx),
                docker: shiplift::Docker::new(),
            }
        });
    }
}

fn main() {
    env_logger::init();

    let system = System::new("Docker-daemon");

    let ars : Vec<String> = std::env::args().collect();
    let socket = ars[1].clone();

    info!("Address is {}", socket.to_string());
    
    let listener = tokio_uds::UnixListener::bind(socket.to_string()).unwrap();
    
    let _server = DockerActorServer::create(move |ctx| {
        ctx.add_message_stream(
            listener
                .incoming()
                .map_err(|_| ())
                .map(|st| lib::UdsConnect(st)),
        );
        DockerActorServer{
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
