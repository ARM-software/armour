use futures::Future;
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;
use tokio_uds::UnixStream;

extern crate cli_lib;
use cli_lib::cli_capnp::cli;

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
    let clicl: cli::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
    runtime.spawn(rpc_system.map_err(|_e| ()));
    {
        let mut request = clicl.read_request();
        request.get().set_a(8);

        let rsp = (runtime.block_on(request.send().promise)).unwrap().get().unwrap().get_r();
        println!("Called to read returned {}", rsp);
    }
}