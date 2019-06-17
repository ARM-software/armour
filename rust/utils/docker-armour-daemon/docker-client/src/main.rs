use futures::Future;
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;
use tokio_uds::UnixStream;

use std::{thread, time};

extern crate docker_lib;
use docker_lib::docker_capnp::docker;

fn main() {
    let args: Vec<String> = ::std::env::args().collect();
    // TODO: add error reporting

    let path = args[1].to_string();
    let mut runtime = current_thread::Runtime::new().unwrap();
    let stream = runtime.block_on(UnixStream::connect(path)).unwrap();

    let (reader, writer) = stream.split();

    let network =
        Box::new(twoparty::VatNetwork::new(reader, std::io::BufWriter::new(writer),
                                           rpc_twoparty_capnp::Side::Client,
                                           Default::default()));
    let mut rpc_system = RpcSystem::new(network, None);
    let dockercl: docker::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
    runtime.spawn(rpc_system.map_err(|_e| ()));
    {
        let listenreq = dockercl.listen_request();
        runtime.block_on(listenreq.send().promise).unwrap().get().unwrap();
        println!("Call to listen returned");

        let mut createnetreq = dockercl.create_network_request();
        createnetreq.get().set_network("armour");
        let res = runtime.block_on(createnetreq.send().promise).unwrap().get().unwrap().get_result();
        println!("Call to create network returned {}", res);

        // thread::sleep(time::Duration::from_secs(1));

        let mut connectreq = dockercl.attach_to_network_request();
        connectreq.get().set_container("armour");
        connectreq.get().set_network("armour");
        let res = runtime.block_on(connectreq.send().promise).unwrap().get().unwrap().get_result();
        println!("Call to attach to network returned {}", res);

        // thread::sleep(time::Duration::from_secs(1));
        
        let mut disconnectreq = dockercl.detach_from_network_request();
        disconnectreq.get().set_container("armour");
        disconnectreq.get().set_network("armour");
        let res = runtime.block_on(disconnectreq.send().promise).unwrap().get().unwrap().get_result();
        println!("Call to detach from network returned {}", res);

        // thread::sleep(time::Duration::from_secs(1));

        let mut deletenetreq = dockercl.remove_network_request();
        deletenetreq.get().set_network("armour");
        let res = runtime.block_on(deletenetreq.send().promise).unwrap().get().unwrap().get_result();;
        println!("Call to delete network returned {}", res);

    }
}
