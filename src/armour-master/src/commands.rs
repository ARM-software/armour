//! Run data plane `master` interactive shell commands

use super::{
    instance::InstanceSelector,
    master::{ArmourDataMaster, Launch, List, PolicyCommand, Quit},
};
use actix::Addr;
use armour_api::proxy::{HttpConfig, LabelOp, PolicyRequest};
use armour_lang::{
    labels,
    policies::{Policies, Protocol},
};
use lazy_static::lazy_static;
use regex::Regex;
use std::io::BufRead;
use std::path::PathBuf;

lazy_static! {
    static ref COMMAND: Regex = Regex::new(
        r"(?x)^(?i)\s*
          (?P<instance>.+:\s+)?
          (?P<command>
            help |
            list |
            quit |
            run |
            wait |
            launch (\s (log | debug))? |
            shutdown |
            status |
            label \s (add | rm) |
            labels \s rm |
            deny \s all |
            allow \s all |
            stop (\s (http | tcp))? |
            policy |
            start \s (http | tcp | ingress) |
            stop (\s (http | tcp))? |
            timeout)
          (?P<arg>\s+.+)?\s*$"
    )
    .unwrap();
}

/// get and parse "instance" block of regular expression capture
fn instance_selector(caps: &regex::Captures) -> Option<InstanceSelector> {
    match caps.name("instance") {
        Some(x) => {
            let s = x.as_str().trim_end().trim_end_matches(':');
            if let Ok(id) = s.parse::<usize>() {
                Some(InstanceSelector::ID(id))
            } else if let Ok(label) = s.parse::<armour_lang::labels::Label>() {
                Some(InstanceSelector::Label(label))
            } else {
                log::warn!("bad instance label: {}", s);
                None
            }
        }
        None => Some(InstanceSelector::All),
    }
}

fn command<'a>(
    caps: &'a regex::Captures,
) -> (Option<InstanceSelector>, Option<&'a str>, Option<&'a str>) {
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
    if instance.is_none() {
        return false;
    };
    let instance = instance.unwrap();
    let command = command.map(|s| s.to_lowercase());
    match (instance == InstanceSelector::All, command.as_deref(), args) {
        (true, Some("help"), None) => println!(
            "COMMANDS:
    help               list commands
    list               list connected instances
    quit               shutdown master and all instances
    run <file>         run commands from <file>
    wait <seconds>     wait for <seconds> to elapse (up to 5s)

    [<id>:] launch [log|debug]         start a new slave instance
    [<id>:] shutdown                   request slave shutdown
    [<id>:] start <proto> <port>       start proxy on <port>
    [<id>:] start http <port> <socket> start ingress proxy for <socket> on <port>
    [<id>:] stop [<proto>]             stop proxy
    [<id>:] status                     retrieve and print status
    [<id>:] timeout <seconds>          set HTTP server response timeout
    
    [<id>:] allow all                  request allow all policy
    [<id>:] deny all                   request deny all policy
    [<id>:] policy  <file>             read policy <file> and send to instance

    [<id>:] label add <host> <label>   add a label
    [<id>:] label rm <host> <label>    remove a label
    [<id>:] labels rm [<host>]         remove labels (for <host> or all)

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
        (_, Some(s @ "launch"), None)
        | (_, Some(s @ "launch log"), None)
        | (_, Some(s @ "launch debug"), None) => {
            let label = match instance {
                InstanceSelector::Label(label) => label,
                InstanceSelector::ID(id) => {
                    log::warn!("{} is not a valid proxy name", id);
                    return false;
                }
                InstanceSelector::All => "proxy".parse().unwrap(),
            };
            let log = if s.ends_with("log") {
                log::Level::Info
            } else if s.ends_with("debug") {
                log::Level::Debug
            } else {
                log::Level::Warn
            };
            master.do_send(Launch::new(label, true, log, None))
        }
        (_, Some("shutdown"), None) => {
            log::info!("sending shudown");
            master.do_send(PolicyCommand::new(instance, PolicyRequest::Shutdown))
        }
        (_, Some("status"), None) => {
            master.do_send(PolicyCommand::new(instance, PolicyRequest::Status))
        }
        (_, Some(s @ "start tcp"), Some(port_socket))
        | (_, Some(s @ "start http"), Some(port_socket)) => {
            if let Ok(port) = port_socket.parse::<u16>() {
                let start = if s.ends_with("http") {
                    PolicyRequest::StartHttp(HttpConfig::Port(port))
                } else {
                    PolicyRequest::StartTcp(port)
                };
                master.do_send(PolicyCommand::new(instance, start))
            } else if s.ends_with("http") {
                match port_socket.split(' ').collect::<Vec<&str>>().as_slice() {
                    [port, socket] => {
                        if let Ok(socket) = socket.parse::<std::net::SocketAddrV4>() {
                            if let Ok(port) = port.parse::<u16>() {
                                let start =
                                    PolicyRequest::StartHttp(HttpConfig::Ingress(port, socket));
                                master.do_send(PolicyCommand::new(instance, start))
                            } else {
                                log::warn!("expecting <port>, got {}", port);
                            }
                        } else {
                            log::warn!("expecting <socket>, got {}", socket);
                        }
                    }
                    _ => log::warn!("expecting <port> [<socket>], got {}", port_socket),
                }
            } else {
                log::warn!("expecting <port>, got {}", port_socket)
            }
        }
        (_, Some("stop"), None) => {
            master.do_send(PolicyCommand::new(
                instance.clone(),
                PolicyRequest::Stop(Protocol::HTTP),
            ));
            master.do_send(PolicyCommand::new(
                instance,
                PolicyRequest::Stop(Protocol::TCP),
            ))
        }
        (_, Some("stop http"), None) => master.do_send(PolicyCommand::new(
            instance,
            PolicyRequest::Stop(Protocol::HTTP),
        )),
        (_, Some("stop tcp"), None) => master.do_send(PolicyCommand::new(
            instance,
            PolicyRequest::Stop(Protocol::TCP),
        )),
        (_, Some("timeout"), Some(secs)) => {
            if let Ok(secs) = secs.parse::<u8>() {
                master.do_send(PolicyCommand::new(instance, PolicyRequest::Timeout(secs)))
            } else {
                log::warn!("timeout <seconds>: expecting u8, got {}", secs);
            }
        }
        (_, Some("policy"), Some(file)) => {
            let path = pathbuf(file);
            match Policies::from_file(&path) {
                Ok(policies) => set_policy(master, instance, policies),
                Err(err) => log::warn!(r#"{:?}: {}"#, path, err),
            }
        }
        (_, Some("allow all"), None) => set_policy(master, instance, Policies::allow_all()),
        (_, Some("deny all"), None) => set_policy(master, instance, Policies::deny_all()),
        (_, Some(s @ "label add"), Some(arg)) | (_, Some(s @ "label rm"), Some(arg)) => {
            if let [key, value] = arg.split(' ').collect::<Vec<&str>>().as_slice() {
                if let Ok(label) = value.parse::<labels::Label>() {
                    if let Ok(ip) = key.parse::<std::net::Ipv4Addr>() {
                        let op = if s.ends_with("add") {
                            LabelOp::AddIp(vec![(ip, label.into())])
                        } else {
                            LabelOp::RemoveIp(ip, Some(label))
                        };
                        master.do_send(PolicyCommand::new(instance, PolicyRequest::Label(op)))
                    } else {
                        let op = if s.ends_with("add") {
                            LabelOp::AddUri(vec![((*key).to_string(), label.into())])
                        } else {
                            LabelOp::RemoveUri((*key).to_string(), Some(label))
                        };
                        master.do_send(PolicyCommand::new(instance, PolicyRequest::Label(op)))
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
                master.do_send(PolicyCommand::new(
                    instance,
                    PolicyRequest::Label(LabelOp::RemoveIp(ip, None)),
                ))
            } else {
                master.do_send(PolicyCommand::new(
                    instance,
                    PolicyRequest::Label(LabelOp::RemoveUri(arg.to_string(), None)),
                ))
            }
        }
        (_, Some("labels rm"), None) => master.do_send(PolicyCommand::new(
            instance,
            PolicyRequest::Label(LabelOp::Clear),
        )),
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

fn set_policy(master: &Addr<ArmourDataMaster>, instance: InstanceSelector, policies: Policies) {
    log::info!("sending policy: {}", policies);
    master.do_send(PolicyCommand::new(
        instance,
        PolicyRequest::SetPolicy(policies),
    ))
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
