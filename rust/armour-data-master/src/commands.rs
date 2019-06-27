//! Data plane master commands
//!
//! The following commands are supported:
//! - `list` : show active connections
//! - `<instance> allow all`
//! - `<instance> deny all`
//! - `<instance> policy <path>` : send a policy to an instance
//! - `<instance> remote <path>` : ask the instance to load a policy from a file

use regex::Regex;

lazy_static! {
    pub static ref LIST: Regex = Regex::new(r"^(?i)\s*list\s*$").unwrap();
}

lazy_static! {
    pub static ref DENY_ALL: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>[[:digit:]]+\s)?\s*deny all\s*$"#).unwrap();
}

lazy_static! {
    pub static ref ALLOW_ALL: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>[[:digit:]]+\s)?\s*allow all\s*$"#).unwrap();
}

lazy_static! {
    pub static ref POLICY: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>[[:digit:]]+\s)?\s*policy\s+"(?P<path>.*)"\s*$"#)
            .unwrap();
}

lazy_static! {
    pub static ref REMOTE: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>[[:digit:]]+\s)?\s*remote\s+"(?P<path>.*)"\s*$"#)
            .unwrap();
}

/// get and parse "instance" block of regular expression capture
pub fn instance(caps: &regex::Captures) -> Option<usize> {
    match caps.name("instance") {
        Some(x) => x.as_str().parse::<usize>().ok(),
        None => None,
    }
}
