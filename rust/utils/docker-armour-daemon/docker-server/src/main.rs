// extern crate capnp;
// #[macro_use] extern crate capnp_rpc;

use futures::{Future, Stream};
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::{AsyncRead};

use tokio_core::reactor;
extern crate docker_lib;
use docker_lib::docker_capnp::docker;

mod docker_api_impl;
use docker_api_impl::DockerImpl;

pub fn main() {
    let args: Vec<String> = ::std::env::args().collect();
    // TODO: add error reporting

    let mut core = reactor::Core::new().unwrap();
    let handle = core.handle();
    
    let path = args[1].to_string();
    let socket = ::tokio_uds::UnixListener::bind(&path).unwrap();

    let docker = docker::ToClient::new(DockerImpl).into_client::<::capnp_rpc::Server>();
    
    let handle1 = handle.clone();
    let done = socket.incoming().for_each(move |socket| {
        let (reader, writer) = socket.split();

        let handle = handle1.clone();

        let net = twoparty::VatNetwork::new(
            reader, 
            std::io::BufWriter::new(writer), 
            rpc_twoparty_capnp::Side::Server, 
            Default::default());

        let rcp_system = RpcSystem::new(Box::new(net), Some(docker.clone().client));
        
        handle.spawn(rcp_system.map_err(|e| println!("error: {:?}", e)));

        Ok(())
    });

    core.run(done).unwrap();
}
