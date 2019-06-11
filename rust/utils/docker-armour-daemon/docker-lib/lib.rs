// extern crate capnp;
// #[macro_use] extern crate capnp_rpc;

pub mod docker_capnp {
    include!(concat!(env!("OUT_DIR"), "/docker_capnp.rs"));
}
