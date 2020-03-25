//! Run data plane `master` interactive shell commands

use super::{
    instance::InstanceSelector,
    master::{ArmourDataMaster, Launch, List, PolicyCommand, Quit},
};
use actix::Addr;
use armour_api::proxy::{LabelOp, Policy, PolicyRequest, Protocol};
use armour_lang::{labels, lang};
use lazy_static::lazy_static;
use regex::Regex;
use std::io::BufRead;
use std::path::PathBuf;

lazy_static! {
    static ref COMMAND: Regex = Regex::new(
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
            label \s (add | rm) |
            labels (\s rm)? |
            deny \s all (\s (http | tcp))?  |
            allow \s all (\s (http | tcp))?  |
            stop (\s (http | tcp))? |
            debug \s (on | off) (\s (http | tcp))? |
            policy (\s (http | tcp))? |
            start \s (http | tcp) |
            stop (\s (http | tcp))? |
            timeout)
          (?P<arg>\s+.+)?\s*$"
    )
    .unwrap();
}

/// get and parse "instance" block of regular expression capture
fn instance_selector(caps: &regex::Captures) -> InstanceSelector {
    match caps.name("instance") {
        Some(x) => {
            let s = x.as_str().trim_end().trim_end_matches(':');
            if let Ok(id) = s.parse::<usize>() {
                InstanceSelector::ID(id)
            } else {
                InstanceSelector::Name(s.to_string())
            }
        }
        None => InstanceSelector::All,
    }
}

fn command<'a>(caps: &'a regex::Captures) -> (InstanceSelector, Option<&'a str>, Option<&'a str>) {
    (
        instance_selector(caps),
        caps.name("command").map(|s| s.as_str()),
        caps.name("arg")
            .map(|s| s.as_str().trim().trim_matches('"')),
    )
}

#[allow(clippy::cognitive_complexity)]
fn master_command(master: &Addr<ArmourDataMaster>, caps: regex::Captures) -> bool {
    let (instance, command, args) = command(&caps);
    let command = command.map(|s| s.to_lowercase());
    match (instance == InstanceSelector::All, command.as_deref(), args) {
        (true, Some("help"), None) => println!(
            "COMMANDS:
    help               list commands
    list               list connected instances
    quit               shutdown master and all instances
    run <file>         run commands from <file>
    wait <seconds>     wait for <seconds> to elapse (up to 5s)

    [<id>:] launch [log]              start a new slave instance
    [<id>:] shutdown                  request slave shutdown
    [<id>:] start <proto> <port>      start proxy on <port>
    [<id>:] stop [<proto>]            stop proxy
    [<id>:] status                    retrieve and print status
    
    [<id>:] allow all [<proto>]       request allow all policy
    [<id>:] deny all [<proto>]        request deny all policy
    [<id>:] policy <proto> <file>     read policy <file> and send to instance

    [<id>:] debug (on|off) [<proto>]  enable/disable debug output
    [<id>:] timeout <seconds>         HTTP server response timeout

    [<id>:] label add <url> <label>   add a label
    [<id>:] label rm <url> <label>    remove a label
    [<id>:] labels                    list labels
    [<id>:] labels rm [<url>]         remove labels (for <url> or all)

    <id>    instance ID number
    <proto> http or tcp"
        ),
        (true, Some("list"), None) => {
            master.do_send(List);
        }
        (true, Some("quit"), None) => {
            master.do_send(Quit);
            return true;
        }
        (true, Some("run"), Some(file)) => run_script(master, file),
        (true, Some("wait"), Some(secs)) => {
            if let Ok(delay) = secs.parse::<u8>() {
                std::thread::sleep(std::time::Duration::from_secs(delay.min(5).into()))
            } else {
                log::warn!("wait <seconds>: expecting u8, got {}", secs);
            }
        }
        (_, Some(s @ "launch"), None) | (_, Some(s @ "launch log"), None) => {
            let name = match instance {
                InstanceSelector::Name(name) => name,
                InstanceSelector::ID(id) => {
                    log::warn!("{} is not a valid proxy name", id);
                    return false;
                }
                InstanceSelector::All => "proxy".to_string(),
            };
            master.do_send(Launch(s.ends_with("log"), name))
        }
        (_, Some("shutdown"), None) => {
            log::info!("sending shudown");
            master.do_send(PolicyCommand(instance, PolicyRequest::Shutdown))
        }
        (_, Some("status"), None) => master.do_send(PolicyCommand(instance, PolicyRequest::Status)),
        (_, Some(s @ "start tcp"), Some(port)) | (_, Some(s @ "start http"), Some(port)) => {
            if let Ok(port) = port.parse::<u16>() {
                let start = if is_http(s) {
                    PolicyRequest::StartHttp(port)
                } else {
                    PolicyRequest::StartTcp(port)
                };
                master.do_send(PolicyCommand(instance, start))
            } else {
                log::warn!("expecting port number, got {}", port);
            }
        }
        (_, Some(s @ "stop"), None)
        | (_, Some(s @ "stop http"), None)
        | (_, Some(s @ "stop tcp"), None) => {
            master.do_send(PolicyCommand(instance, PolicyRequest::Stop(protocol(s))))
        }
        (_, Some(s @ "debug on"), None)
        | (_, Some(s @ "debug on http"), None)
        | (_, Some(s @ "debug on tcp"), None) => master.do_send(PolicyCommand(
            instance,
            PolicyRequest::Debug(protocol(s), true),
        )),
        (_, Some(s @ "debug off"), None)
        | (_, Some(s @ "debug off http"), None)
        | (_, Some(s @ "debug off tcp"), None) => master.do_send(PolicyCommand(
            instance,
            PolicyRequest::Debug(protocol(s), false),
        )),
        (_, Some("timeout"), Some(secs)) => {
            if let Ok(secs) = secs.parse::<u8>() {
                master.do_send(PolicyCommand(instance, PolicyRequest::Timeout(secs)))
            } else {
                log::warn!("timeout <seconds>: expecting u8, got {}", secs);
            }
        }
        (_, Some(s @ "policy http"), Some(file)) | (_, Some(s @ "policy tcp"), Some(file)) => {
            let path = pathbuf(file);
            let protocol = protocol(s);
            match lang::Module::from_file(&path, Some(protocol.interface())) {
                Ok(module) => set_policy(master, instance, Policy::Program(module.program)),
                Err(err) => log::warn!(r#"{:?}: {}"#, path, err),
            }
        }
        (_, Some(s @ "allow all"), None)
        | (_, Some(s @ "allow all http"), None)
        | (_, Some(s @ "allow all tcp"), None)
        | (_, Some(s @ "deny all"), None)
        | (_, Some(s @ "deny all http"), None)
        | (_, Some(s @ "deny all tcp"), None) => {
            let protocol = protocol(s);
            if s.starts_with("allow") {
                set_policy(master, instance, Policy::AllowAll(protocol))
            } else {
                set_policy(master, instance, Policy::DenyAll(protocol))
            }
        }
        (_, Some(s @ "label add"), Some(arg)) | (_, Some(s @ "label rm"), Some(arg)) => {
            if let [key, value] = arg.split(' ').collect::<Vec<&str>>().as_slice() {
                if let Ok(label) = value.parse::<labels::Label>() {
                    if let Ok(ip) = key.parse::<std::net::Ipv4Addr>() {
                        let op = if s.ends_with("add") {
                            LabelOp::AddIp(ip, label)
                        } else {
                            LabelOp::RemoveIp(ip, Some(label))
                        };
                        master.do_send(PolicyCommand(instance, PolicyRequest::Label(op)))
                    } else if let Ok(url) = key.parse::<url::Url>() {
                        let op = if s.ends_with("add") {
                            LabelOp::AddUrl(url, label)
                        } else {
                            LabelOp::RemoveUrl(url, Some(label))
                        };
                        master.do_send(PolicyCommand(instance, PolicyRequest::Label(op)))
                    } else {
                        log::info!("expecting <url> or <ip>")
                    }
                } else {
                    log::info!("expecting <label>")
                }
            } else {
                log::info!("expecting <url> or <ip>, and <label>")
            }
        }
        (_, Some("labels rm"), Some(arg)) => {
            if let Ok(ip) = arg.parse::<std::net::Ipv4Addr>() {
                master.do_send(PolicyCommand(
                    instance,
                    PolicyRequest::Label(LabelOp::RemoveIp(ip, None)),
                ))
            } else if let Ok(url) = arg.parse::<url::Url>() {
                master.do_send(PolicyCommand(
                    instance,
                    PolicyRequest::Label(LabelOp::RemoveUrl(url, None)),
                ))
            } else {
                log::info!("expecting <url> or <ip>")
            }
        }
        (_, Some("labels rm"), None) => master.do_send(PolicyCommand(
            instance,
            PolicyRequest::Label(LabelOp::Clear),
        )),
        (_, Some("labels"), None) => {
            master.do_send(PolicyCommand(instance, PolicyRequest::Label(LabelOp::List)))
        }
        _ => log::info!("unknown command"),
    }
    false
}

pub fn run_command(master: &Addr<ArmourDataMaster>, cmd: &str) -> bool {
    let cmd = cmd.trim();
    if cmd != "" {
        if let Some(caps) = COMMAND.captures(&cmd) {
            return master_command(&master, caps);
        } else {
            log::info!("unknown command <none>")
        }
    }
    false
}

pub fn run_script(master: &Addr<ArmourDataMaster>, script: &str) {
    match std::fs::File::open(pathbuf(script)) {
        Ok(file) => {
            let mut buf_reader = std::io::BufReader::new(file);
            let mut line = 1;
            let mut done = false;
            while !done {
                let mut cmd = String::new();
                if let Ok(res) = buf_reader.read_line(&mut cmd) {
                    cmd = cmd.trim().to_string();
                    if !(cmd == "" || cmd.starts_with('#')) {
                        log::info!(r#"run command: "{}""#, cmd);
                        if run_command(master, &cmd) {
                            return;
                        }
                    };
                    line += 1;
                    done = res == 0
                } else {
                    log::warn!("{}: error reading command on line {}", script, line);
                    done = true
                }
            }
        }
        Err(err) => log::warn!("{}: {}", script, err),
    }
}

fn is_http(s: &str) -> bool {
    s.ends_with("http")
}

fn is_tcp(s: &str) -> bool {
    s.ends_with("tcp")
}

fn protocol(s: &str) -> Protocol {
    if is_http(s) {
        Protocol::HTTP
    } else if is_tcp(s) {
        Protocol::TCP
    } else {
        Protocol::All
    }
}

fn set_policy(master: &Addr<ArmourDataMaster>, instance: InstanceSelector, policy: Policy) {
    log::info!("sending policy: {}", policy);
    master.do_send(PolicyCommand(instance, PolicyRequest::SetPolicy(policy)))
}

fn pathbuf(s: &str) -> std::path::PathBuf {
    PathBuf::from(s)
        .iter()
        .map(|oss| {
            oss.to_str()
                .map(|s| s.replace("\\ ", " ").into())
                .unwrap_or_else(|| oss.to_os_string())
        })
        .collect()
}
