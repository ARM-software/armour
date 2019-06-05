extern crate capnp;
#[macro_use]
extern crate capnp_rpc;

use futures::{Future, Stream};
use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};
use tokio::io::{AsyncRead};
// This should be single threaded
use tokio::runtime::current_thread;

extern crate iptables_lib;
use iptables_lib::iptables_capnp::iptables;

mod iptables_impl;
use iptables_impl::IptablesImpl;

extern crate nix;
use nix::unistd::{chown, Uid, Gid};
use std::fs::set_permissions;
use std::os::unix::fs::PermissionsExt;

pub fn main() {
    let args: Vec<String> = ::std::env::args().collect();
    // TODO: add error reporting
   
    let path = args[1].to_string();
    let socket = ::tokio_uds::UnixListener::bind(&path).unwrap();
    chown(&*path, None, Some(Gid::from_raw(100))).unwrap();
    set_permissions(&*path, PermissionsExt::from_mode(496));
    
    let iptables = iptables::ToClient::new(IptablesImpl).into_client::<::capnp_rpc::Server>();

    let done = socket.incoming().for_each(move |socket| {
        let (reader, writer) = socket.split();

        let net = twoparty::VatNetwork::new(
            reader, 
            std::io::BufWriter::new(writer), 
            rpc_twoparty_capnp::Side::Server, 
            Default::default());

        let rcp_system = RpcSystem::new(Box::new(net), Some(iptables.clone().client));
        current_thread::spawn(rcp_system.map_err(|e| println!("error: {:?}", e)));
        Ok(())
    });

    current_thread::block_on_all(done).unwrap();
}
