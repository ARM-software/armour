/// Communication interface between data plane master and proxy instances

#[macro_use]
extern crate lazy_static;

use armour_policy::{lang::Interface, types::Typ};
use std::collections::HashSet;
use std::net::IpAddr;

pub mod codec;

pub const ALLOW_REST_REQUEST: &str = "allow_rest_request";
pub const ALLOW_CLIENT_PAYLOAD: &str = "allow_client_payload";
pub const ALLOW_SERVER_PAYLOAD: &str = "allow_server_payload";
pub const ALLOW_REST_RESPONSE: &str = "allow_rest_response";
pub const ALLOW_TCP_CONNECTION: &str = "allow_tcp_connection";
pub const ON_TCP_DISCONNECT: &str = "on_tcp_disconnect";

lazy_static! {
    pub static ref REST_POLICY: Interface = {
        let mut policy = Interface::new();
        policy.insert_bool(ALLOW_REST_REQUEST,
            vec![
                vec![Typ::HttpRequest, Typ::ID, Typ::ID, Typ::I64],
                vec![Typ::HttpRequest, Typ::ID, Typ::ID],
                vec![Typ::HttpRequest],
                Vec::new(),
            ],
            false, // default to disallow
        );
        policy.insert_bool(ALLOW_CLIENT_PAYLOAD,
            vec![
                vec![Typ::Data, Typ::ID, Typ::ID, Typ::I64],
                vec![Typ::Data, Typ::ID, Typ::ID],
                vec![Typ::Data],
            ],
            true, // default to allow
        );
        policy.insert_bool(ALLOW_SERVER_PAYLOAD,
            vec![
                vec![Typ::Data, Typ::ID, Typ::ID, Typ::I64],
                vec![Typ::Data, Typ::ID, Typ::ID],
                vec![Typ::Data],
            ],
            true, // default to allow
        );
        policy.insert_bool(ALLOW_REST_RESPONSE,
            vec![
                vec![Typ::HttpResponse, Typ::ID, Typ::ID, Typ::I64],
                vec![Typ::HttpResponse, Typ::ID, Typ::ID],
                vec![Typ::HttpResponse],
                Vec::new(),
            ],
            true, // default to allow
        );
        policy
    };
    pub static ref TCP_POLICY: Interface = {
        let mut policy = Interface::new();
        policy.insert_bool(ALLOW_TCP_CONNECTION,
            vec![
                vec![Typ::ID, Typ::ID, Typ::I64],
                vec![Typ::ID, Typ::ID],
                Vec::new(),
            ],
            false, // default to disallow
        );
        policy.insert_unit(ON_TCP_DISCONNECT,
            vec![
                vec![Typ::ID, Typ::ID, Typ::I64, Typ::I64, Typ::I64],
                vec![Typ::ID, Typ::ID, Typ::I64],
                vec![Typ::ID, Typ::ID],
            ]
        );
        policy
    };
    pub static ref TCP_REST_POLICY: Interface = {
        let mut policy = TCP_POLICY.clone();
        policy.extend(&REST_POLICY);
        policy
    };
}

lazy_static! {
    pub static ref INTERFACE_IPS: HashSet<IpAddr> = {
        let set: HashSet<String> = ["lo", "lo0", "en0", "eth0"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
            interfaces
                .into_iter()
                .filter_map(|i| {
                    if set.contains(&i.name) {
                        Some(i.ip())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            HashSet::new()
        }
    };
}

pub fn own_ip(s: &IpAddr) -> bool {
    INTERFACE_IPS.contains(s)
        || match s {
            IpAddr::V4(v4) => v4.is_unspecified() || v4.is_broadcast(),
            IpAddr::V6(v6) => v6.is_unspecified(),
        }
}
