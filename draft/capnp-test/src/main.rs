extern crate capnp;
#[macro_use]
extern crate capnp_rpc;
extern crate futures;
extern crate tokio;

pub mod oracle_capnp {
    include!(concat!(env!("OUT_DIR"), "/oracle_capnp.rs"));
}

pub mod client;
pub mod server;

fn main() -> Result<(), ::capnp::Error> {
    let args: Vec<String> = ::std::env::args().collect();
    if args.len() >= 2 {
        match &args[1][..] {
            "client" => return client::main(),
            "server" => return server::main(),
            _ => (),
        }
    }

    println!("usage: {} [client | server] ADDRESS", args[0]);
    Ok(())
}
