//! Data plane master commands
//!
//! The following commands are supported:
//! - `list` : show active connections
//! - `<instance> allow all`
//! - `<instance> deny all`
//! - `<instance> policy <path>` : send a policy to an instance
//! - `<instance> remote <path>` : ask the instance to load a policy from a file

use super::InstanceSelector;
use regex::Regex;

lazy_static! {
    pub static ref COMMAND: Regex = Regex::new(
        r"(?x)^(?i)\s*
          (?P<instance>[[:alnum:]]+:\s+)?
          (?P<command>
            help |
            list |
            quit |
            run |
            wait |
            launch (\s log)? |
            shutdown |
            status |
            ((http | tcp) \s)? deny \s all |
            ((http | tcp) \s)? allow \s all |
            ((http | tcp) \s)? stop |
            ((http | tcp) \s)? debug \s (on | off) |
            ((http | tcp) \s)? policy |
            (http | tcp) \s start |
            ((http | tcp) \s)? stop |
            http \s timeout)
          (?P<arg>\s+.+)?\s*$"
    )
    .unwrap();
}

/// get and parse "instance" block of regular expression capture
fn instance_selector(caps: &regex::Captures) -> InstanceSelector {
    match caps.name("instance") {
        Some(x) => x
            .as_str()
            .trim_end()
            .trim_end_matches(':')
            .parse::<usize>()
            .map(InstanceSelector::ID)
            .unwrap_or(InstanceSelector::Error),
        None => InstanceSelector::All,
    }
}

pub fn command<'a>(
    caps: &'a regex::Captures,
) -> (InstanceSelector, Option<&'a str>, Option<&'a str>) {
    (
        instance_selector(caps),
        caps.name("command").map(|s| s.as_str()),
        caps.name("arg")
            .map(|s| s.as_str().trim().trim_matches('"')),
    )
}
