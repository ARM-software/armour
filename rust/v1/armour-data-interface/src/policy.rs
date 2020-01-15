/// Policy interfaces
use armour_policy::{lang::Interface, types::Typ};

pub const ALLOW_REST_REQUEST: &str = "allow_rest_request";
pub const ALLOW_CLIENT_PAYLOAD: &str = "allow_client_payload";
pub const ALLOW_SERVER_PAYLOAD: &str = "allow_server_payload";
pub const ALLOW_REST_RESPONSE: &str = "allow_rest_response";
pub const ALLOW_TCP_CONNECTION: &str = "allow_tcp_connection";
pub const ON_TCP_DISCONNECT: &str = "on_tcp_disconnect";

lazy_static! {
    pub static ref REST_POLICY: Interface = {
        let mut policy = Interface::new();
        policy.insert_bool(ALLOW_REST_REQUEST, vec![vec![Typ::HttpRequest], Vec::new()]);
        policy.insert_bool(ALLOW_CLIENT_PAYLOAD, vec![vec![Typ::Payload]]);
        policy.insert_bool(ALLOW_SERVER_PAYLOAD, vec![vec![Typ::Payload]]);
        policy.insert_bool(
            ALLOW_REST_RESPONSE,
            vec![vec![Typ::HttpResponse], Vec::new()],
        );
        policy
    };
    pub static ref TCP_POLICY: Interface = {
        let mut policy = Interface::new();
        policy.insert_bool(
            ALLOW_TCP_CONNECTION,
            vec![vec![Typ::Connection], Vec::new()],
        );
        policy.insert_unit(
            ON_TCP_DISCONNECT,
            vec![
                vec![Typ::Connection, Typ::I64, Typ::I64],
                vec![Typ::Connection],
                Vec::new(),
            ],
        );
        policy
    };
    pub static ref TCP_REST_POLICY: Interface = {
        let mut policy = TCP_POLICY.clone();
        policy.extend(&REST_POLICY);
        policy
    };
}
