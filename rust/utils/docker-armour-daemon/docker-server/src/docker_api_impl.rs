use tokio::runtime::current_thread::TaskExecutor;
use futures::future::Executor;
use futures::{Future, Stream};


use capnp::capability::Promise;

extern crate docker_lib;
use docker_lib::docker_capnp::docker;

extern crate shiplift;
use shiplift::Docker;

pub struct DockerImpl;
impl docker::Server for DockerImpl {
    fn listen(&mut self, params: docker::ListenParams, mut results: docker::ListenResults) -> 
        Promise<(), ::capnp::Error>
    { 

        let docker = Docker::new();
        println!("listening for events");

        let fut = docker.events(&Default::default())
            .for_each(|e| {
                println!("event -> {:?}", e);
                Ok(())
            })
            .map_err(|e| eprintln!("Error: {}", e));
        
        TaskExecutor::current().execute(fut).unwrap();       

        Promise::ok(())
    }
}
