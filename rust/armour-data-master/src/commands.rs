//! Data plane master commands
//!
//! The following commands are supported:
//! - `list` : show active connections
//! - `<instance> allow all`
//! - `<instance> deny all`
//! - `<instance> policy <path>` : send a policy to an instance
//! - `<instance> remote <path>` : ask the instance to load a policy from a file

use super::Instances;
use regex::Regex;

lazy_static! {
    pub static ref LIST: Regex = Regex::new(r"^(?i)\s*list\s*$").unwrap();
}

lazy_static! {
    pub static ref DENY_ALL: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?deny all\s*$"#).unwrap();
}

lazy_static! {
    pub static ref ALLOW_ALL: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?allow all\s*$"#).unwrap();
}

lazy_static! {
    pub static ref SHUTDOWN: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?shutdown\s*$"#).unwrap();
}

lazy_static! {
    pub static ref POLICY: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?policy\s+"(?P<path>.*)"\s*$"#)
            .unwrap();
}

lazy_static! {
    pub static ref REMOTE: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?remote\s+"(?P<path>.*)"\s*$"#)
            .unwrap();
}

/// get and parse "instance" block of regular expression capture
pub fn instance(caps: &regex::Captures) -> Instances {
    match caps.name("instance") {
        Some(x) => {
            let s = x.as_str().trim();
            if s.to_lowercase() == "all" {
                Instances::All
            } else {
                s.parse::<usize>()
                    .map(|i| Instances::ID(i))
                    .unwrap_or(Instances::Error)
            }
        }
        None => Instances::SoleInstance,
    }
}
