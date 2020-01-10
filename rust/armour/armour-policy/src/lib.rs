//! Armour policy language

#[cfg(unix)]
extern crate capnp;
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

/// Language AST
pub mod expressions;
/// Make calls to external security services
///
/// For example, external services can be used for logging and session management
pub mod externals;
/// Record the types of built-in and user functions
pub mod headers;
/// Policy language interpreter
pub mod interpret;
/// Language interface
pub mod lang;
/// Lexer implemented using [nom](../nom/index.html)
pub mod lexer;
/// Armour primitive types
pub mod literals;
/// Parser implemented using [nom](../nom/index.html)
pub mod parser;
/// Pretty-printer
pub mod pretty;
/// Type system
pub mod types;
