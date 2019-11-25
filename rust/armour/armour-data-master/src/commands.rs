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
    pub static ref MASTER: Regex = Regex::new(
        r"(?x)^(?i)\s*(?P<command>
            help |
            list |
            launch (\s log)? |
            quit
            )\s*$"
    )
    .unwrap();
}

lazy_static! {
    pub static ref INSTANCE0: Regex = Regex::new(
        r#"(?x)^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?(?P<command>
            ((http | tcp) \s)? deny \s all |
            ((http | tcp) \s)? allow \s all |
            shutdown |
            ((http | tcp) \s)? stop |
            ((http | tcp) \s)? debug \s (on | off) |
            status
            )\s*$"#
    )
    .unwrap();
}

lazy_static! {
    pub static ref INSTANCE1: Regex = Regex::new(
        r#"(?x)^(?i)\s*(?P<instance>([[:digit:]]+|all)\s+)?(?P<command>
            policy |
            (http | tcp) \s start |
            ((http | tcp) \s)? stop |
            http \s timeout |
            run |
            wait
            )\s+(?P<arg>.*)\s*$"#
    )
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
                    .map(Instances::ID)
                    .unwrap_or(Instances::Error)
            }
        }
        None => Instances::SoleInstance,
    }
}
