use futures::Future;
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;
use tokio_uds::UnixStream;

extern crate iptables_lib;
use iptables_lib::iptables_capnp::iptables;

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
    let iptablescl: iptables::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
    runtime.spawn(rpc_system.map_err(|_e| ()));
    {
        let mut request = iptablescl.newchain_request();
        request.get().set_table("nat");
        request.get().set_chain("NEWCHAIN");
        let result_new = request.send();
        let mut request = iptablescl.deletechain_request();
        request.get().set_table("nat");
        request.get().set_chain("NEWCHAIN");
        let result_del = request.send();
        
        let rsp1 = (runtime.block_on(result_new.promise)).unwrap().get().unwrap().get_result();
        let rsp2 = (runtime.block_on(result_del.promise)).unwrap().get().unwrap().get_result();
        println!("result 1 is {:?}, result 2 is {:?}", rsp1, rsp2);
    }
}
