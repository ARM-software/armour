#[macro_use]
extern crate capnp_rpc;

use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::Future;
use tokio::io::AsyncRead;

use controlplane_lib::controlplane_capnp::control_plane_proto;

// use docker_lib::docker_capnp::docker;
mod controlplane_proto_impl;
use controlplane_proto_impl::ControlPlaneProtoImpl;

use actix::prelude::*;


fn main() {
    let args: Vec<String> = ::std::env::args().collect();
    // TODO: add error reporting

    let sys = System::new("");

    let path = args[1].to_string();
    let socket = ::tokio_uds::UnixListener::bind(&path).unwrap();

    let done = socket
        .incoming()
        .for_each(move |socket| {
            let (reader, writer) = socket.split();

            let net = twoparty::VatNetwork::new(
                reader,
                std::io::BufWriter::new(writer),
                rpc_twoparty_capnp::Side::Server,
                Default::default(),
            );

            let cpn = control_plane_proto::ToClient::new(ControlPlaneProtoImpl)
                .into_client::<::capnp_rpc::Server>();
            
            let rpc = Some(cpn.clone().client);
            let rpc_system = RpcSystem::new(Box::new(net), rpc);

            actix::spawn(rpc_system.map_err(|e| println!("error: {:?}", e)));
            // Ok()
            futures::future::ok(())
        })
    .map(|_| ())
    .map_err(|_| ());

    actix::spawn(done);
    let _ = sys.run();
}
