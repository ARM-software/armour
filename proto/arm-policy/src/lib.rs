#[macro_use]
extern crate lazy_static;
extern crate capnp;
#[macro_use]
extern crate capnp_rpc;

pub mod external_capnp {
    include!(concat!(env!("OUT_DIR"), "/external_capnp.rs"));
}

pub mod externals;
pub mod interpret;
pub mod lang;
pub mod lexer;
pub mod literals;
pub mod parser;
pub mod types;
