#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use actix::prelude::*;

#[derive(Message)]
pub struct Stop;

pub mod http_proxy;
pub mod policy;
pub mod tcp_proxy;
