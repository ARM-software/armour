extern crate capnp;
#[macro_use]
extern crate capnp_rpc;

use capnp::capability::Promise;

use futures::{Future, Stream};
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::{AsyncRead};
use tokio_core::reactor;
// use tokio::runtime::current_thread;

extern crate cli_lib;
use cli_lib::cli_capnp::cli;

struct CliImpl;
impl cli::Server for CliImpl {
    fn read(&mut self, params: cli::ReadParams, mut results: cli::ReadResults) -> 
        Promise<(), ::capnp::Error>
    { 
        let a = pry!(params.get()).get_a();
        results.get().set_r(a);
        // println!("call to read");
        Promise::ok(())
    }
}

pub fn main() {
    let args: Vec<String> = ::std::env::args().collect();
    // TODO: add error reporting

    let mut core = reactor::Core::new().unwrap();
    let handle = core.handle();
    
    let path = args[1].to_string();
    let socket = ::tokio_uds::UnixListener::bind(&path).unwrap();

    let cli = cli::ToClient::new(CliImpl).into_client::<::capnp_rpc::Server>();

    let handle1 = handle.clone();
    let done = socket.incoming().for_each(move |socket| {
        let (reader, writer) = socket.split();

        let handle = handle1.clone();

        let net = twoparty::VatNetwork::new(
            reader, 
            std::io::BufWriter::new(writer), 
            rpc_twoparty_capnp::Side::Server, 
            Default::default());

        let rcp_system = RpcSystem::new(Box::new(net), Some(cli.clone().client));
        handle.spawn(rcp_system.map_err(|e| println!("error: {:?}", e)));
        Ok(())
    });

    core.run(done).unwrap();
}