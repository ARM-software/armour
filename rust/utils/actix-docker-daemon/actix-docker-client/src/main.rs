use actix::System;
use actix::prelude::*;
use std::time::{Duration, Instant};

#[macro_use]
extern crate log;
extern crate env_logger;

use docker_lib as lib;

fn main() {

    env_logger::init();

    let system = System::new("docker-client");

    let ars : Vec<String> = std::env::args().collect();
    let socket = ars[1].clone();
    let cp_socket = ars[2].clone();

    info!("Address is {}", socket.to_string());

    let addr = lib::DockerClientActor::create_docker_client(socket.to_string()).unwrap();
    
    let fut = {
        futures::future::ok(((), addr))
            .and_then(move |addr| {
                let f = addr.1.send(lib::DockerCommands::Listen{socket_name: cp_socket.to_string()});
                futures::future::ok((f, addr.1))
            })
            .and_then(|addr|
                      tokio::timer::Delay::new(Instant::now() + Duration::from_secs(5))
                      .map_err(|e| panic!("timer failed; err={:?}", e))
                      .map(|_| addr)
            )
            .and_then(|addr| {
                let f = addr.1.send(lib::DockerCommands::CreateNetwork{network: "armour1".to_string()});
                futures::future::ok((f, addr.1))
            })
            .and_then(|addr|
                      tokio::timer::Delay::new(Instant::now() + Duration::from_secs(5))
                      .map_err(|e| panic!("timer failed; err={:?}", e))
                      .map(|_| addr)
            )
            .and_then(|addr| {
                let f = addr.1.send(lib::DockerCommands::CreateNetwork{network: "armour".to_string()});
                futures::future::ok((f, addr.1))
            })
            .and_then(|addr|
                      tokio::timer::Delay::new(Instant::now() + Duration::from_secs(10))
                      .map_err(|e| panic!("timer failed; err={:?}", e))
                      .map(|_| addr)
            )
            .and_then(|addr| {
                let f = addr.1.send(lib::DockerCommands::DeleteNetwork{network: "armour".to_string()});
                futures::future::ok((f, addr.1))
            })
            .and_then(|addr|
                      tokio::timer::Delay::new(Instant::now() + Duration::from_secs(5))
                      .map_err(|e| panic!("timer failed; err={:?}", e))
                      .map(|_| addr)
            )
            .and_then(|addr| {
                let f = addr.1.send(
                    lib::DockerCommands::AttachToNetwork{ container: "armour".to_string(), network: "armour1".to_string() }
                );
                futures::future::ok((f, addr.1))
            })
            .and_then(|addr|
                      tokio::timer::Delay::new(Instant::now() + Duration::from_secs(5))
                      .map_err(|e| panic!("timer failed; err={:?}", e))
                      .map(|_| addr)
            )
            .and_then(|addr| {
                let f = addr.1.send(
                    lib::DockerCommands::DetachFromNetwork{ container: "armour".to_string(), network: "armour1".to_string() }
                );
                futures::future::ok((f,addr.1))
            }
            )
            .and_then(|addr|
                      tokio::timer::Delay::new(Instant::now() + Duration::from_secs(5))
                      .map_err(|e| panic!("timer failed; err={:?}", e))
                      .map(|_| addr)
            )
            .and_then(|addr| addr.1.send( lib::DockerCommands::DeleteNetwork{ network: "armour1".to_string() } ))
            .map(|_| ()) 
            .map_err(|e| info!("Error {:?}", e))
    };
        
    actix::spawn(fut);
    
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
