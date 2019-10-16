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
    pub static ref MASTER: Regex =
        Regex::new(r"^(?i)\s*(?P<command>help|list|launch|launch log|quit)\s*$").unwrap();
}

lazy_static! {
    pub static ref INSTANCE0: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?(?P<command>deny all|allow all|shutdown|stop all|debug on|debug off|status)\s*$"#).unwrap();
}

lazy_static! {
    pub static ref INSTANCE1: Regex =
        Regex::new(r#"^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?(?P<command>print|policy|http start|tcp start|http stop|tcp stop|wait|http timeout|run)\s+(?P<arg>.*)\s*$"#)
            .unwrap();
}

lazy_static! {
    pub static ref COMMANDS: Regex =
        Regex::new(r"(?x)^(?i)\s*(?P<command>help|list|launch|launch log|quit)\s*$").unwrap();
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
                    .map(Instances::ID)
                    .unwrap_or(Instances::Error)
            }
        }
        None => Instances::SoleInstance,
    }
}
