// extern crate capnp;
// #[macro_use] extern crate capnp_rpc;

pub mod iptables_capnp {
    include!(concat!(env!("OUT_DIR"), "/iptables_capnp.rs"));
}
