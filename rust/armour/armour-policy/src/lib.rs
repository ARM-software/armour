//! Armour policy language

extern crate capnp;
#[macro_use]
extern crate capnp_rpc;
#[macro_use]
extern crate enum_display_derive;
#[macro_use]
extern crate lazy_static;
extern crate log;

/// Cap'n Proto interface used by [externals](externals/index.html)
pub mod external_capnp {
    include!(concat!(env!("OUT_DIR"), "/external_capnp.rs"));
}

/// Make calls to external security services
///
/// For example, external services can be used for logging and session management
pub mod externals;
/// Record the types of built-in and user functions
pub mod headers;
/// Policy language interpreter
pub mod interpret;
/// Language AST and interface
pub mod lang;
/// Lexer implemented using [nom](../nom/index.html)
pub mod lexer;
/// Armour primitive types
pub mod literals;
/// Parser implemented using [nom](../nom/index.html)
pub mod parser;
/// Type system
pub mod types;
