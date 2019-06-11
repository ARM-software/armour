use futures::Future;
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;
use tokio_uds::UnixStream;

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
        let request = dockercl.listen_request();
        runtime.block_on(request.send().promise).unwrap();
        println!("Call to listen returned");
    }
}
