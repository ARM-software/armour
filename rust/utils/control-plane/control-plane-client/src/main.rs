use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::Future;
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;
use tokio_uds::UnixStream;

use std::{thread, time};

use controlplane_lib::controlplane_capnp::control_plane_proto;

extern crate docker_lib;
use docker_lib::docker_capnp::docker;

fn main() {
    let args: Vec<String> = ::std::env::args().collect();
    // TODO: add error reporting

    let path = args[1].to_string();
    let mut runtime = current_thread::Runtime::new().unwrap();
    let stream = runtime.block_on(UnixStream::connect(path)).unwrap();

    let (reader, writer) = stream.split();

    let network = Box::new(twoparty::VatNetwork::new(
        reader,
        std::io::BufWriter::new(writer),
        rpc_twoparty_capnp::Side::Client,
        Default::default(),
    ));
    let mut rpc_system = RpcSystem::new(network, None);
    let cpctl: control_plane_proto::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
    runtime.spawn(rpc_system.map_err(|_e| ()));
    {
        let mut event = cpctl.receive_docker_event_request();
        event
            .get()
            .set_event(&"this is not really an event".to_string());
        runtime.block_on(event.send().promise).unwrap().get();
        println!("sent a simple event");
    }
}
