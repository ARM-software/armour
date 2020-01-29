//! Data plane master commands
//!
//! The following commands are supported:
//! - `list` : show active connections
//! - `<instance> allow all`
//! - `<instance> deny all`
//! - `<instance> policy <path>` : send a policy to an instance
//! - `<instance> remote <path>` : ask the instance to load a policy from a file

use super::{
    instance::InstanceSelector,
    master::{AddChild, ArmourDataMaster, MasterCommand},
};
use actix::Addr;
use armour_api::proxy::{PolicyRequest, Protocol};
use armour_lang::lang;
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

fn master_command(
    master: &Addr<ArmourDataMaster>,
    caps: regex::Captures,
    socket: &std::path::PathBuf,
) -> bool {
    let (instance, command, args) = command(&caps);
    let command = command.map(|s| s.to_lowercase());
    match (
        instance == InstanceSelector::All,
        command.as_ref().map(String::as_str),
        args,
    ) {
        (true, Some("help"), None) => println!(
            "COMMANDS:
    help                      list commands
    list                      list connected instances
    quit                      shutdown master and all instances
    run <file>                run commands from <file>
    wait <seconds>            wait for <seconds> to elapse (up to 10s)
    <id> launch [log]         start a new slave instance

    [<id>] shutdown                   request slave shutdown
    [<id>] status                     retrieve and print status
    [<id>] [http|tcp] start <port>    start HTTP/TCP proxy on <port>
    [<id>] [http|tcp] stop            stop HTTP/TCP proxy
    [<id>] [http|tcp] allow all       request allow all policy
    [<id>] [http|tcp] deny all        request deny all policy
    [<id>] [http|tcp] policy <file>   read policy <file> and send to instance
    [<id>] [http|tcp] debug [on|off]  enable/disable display of HTTP requests
    [<id>] http timeout <seconds>     server response timeout
    
    <id>  instance ID number"
        ),
        (true, Some("list"), None) => {
            master.do_send(MasterCommand::ListActive);
        }
        (true, Some("quit"), None) => {
            master.do_send(MasterCommand::Quit);
            return true;
        }
        (true, Some("run"), Some(file)) => run_script(file, master, socket),
        (true, Some("wait"), Some(secs)) => {
            if let Ok(delay) = secs.parse::<u8>() {
                std::thread::sleep(std::time::Duration::from_secs(delay.min(10).into()))
            } else {
                log::warn!("wait <seconds>: expecting u8, got {}", secs);
            }
        }
        (_, Some(s @ "launch"), None) | (_, Some(s @ "launch log"), None) => {
            let log = if s.ends_with("log") { "info" } else { "warn" };
            let armour_proxy = armour_proxy();
            let name = match instance {
                InstanceSelector::Name(name) => name,
                InstanceSelector::ID(id) => {
                    log::warn!("{} is not a valid proxy name", id);
                    return false;
                }
                InstanceSelector::All => "proxy".to_string(),
            };
            // sudo ~/.cargo/bin/flamegraph ~/rust/target/debug/armour-proxy
            match std::process::Command::new(&armour_proxy)
                .arg("-l")
                .arg(log)
                .arg("-n")
                .arg(&name)
                .arg(socket)
                .spawn()
            {
                Ok(child) => {
                    let pid = child.id();
                    master.do_send(AddChild(pid, child));
                    log::info!("launched proxy processs: {} {}", name, pid)
                }
                Err(err) => log::warn!("failed to launch: {}\n{}", armour_proxy.display(), err),
            }
            // let mut command = std::process::Command::new("sudo");
            // let command = command
            //     .arg("/Users/antfox02/.cargo/bin/flamegraph")
            //     .arg("/Users/antfox02/rust/target/release/armour-proxy")
            //     .arg("armour")
            //     .arg(log);
            // log::info!("command: {:?}", command);
            // match command.spawn() {
            //     Ok(child) => log::info!("started processs: {}", child.id()),
            //     Err(err) => log::warn!("failed to spawn data plane instance: {}", err),
            // }
        }
        (_, Some("shutdown"), None) => {
            log::info!("sending shudown");
            master.do_send(MasterCommand::UpdatePolicy(
                instance,
                Box::new(PolicyRequest::Shutdown),
            ))
        }
        (_, Some("status"), None) => master.do_send(MasterCommand::UpdatePolicy(
            instance,
            Box::new(PolicyRequest::Status),
        )),
        (_, Some(s @ "tcp start"), Some(port)) | (_, Some(s @ "http start"), Some(port)) => {
            if let Ok(port) = port.parse::<u16>() {
                let start = if is_rest(s) {
                    PolicyRequest::StartHttp(port)
                } else {
                    PolicyRequest::StartTcp(port)
                };
                master.do_send(MasterCommand::UpdatePolicy(instance, Box::new(start)))
            } else {
                log::warn!("tcp start <port>: expecting port number, got {}", port);
            }
        }
        (_, Some(s @ "stop"), None)
        | (_, Some(s @ "http stop"), None)
        | (_, Some(s @ "tcp stop"), None) => master.do_send(MasterCommand::UpdatePolicy(
            instance,
            Box::new(PolicyRequest::Stop(protocol(s))),
        )),
        (_, Some(s @ "debug on"), None)
        | (_, Some(s @ "http debug on"), None)
        | (_, Some(s @ "tcp debug on"), None) => master.do_send(MasterCommand::UpdatePolicy(
            instance,
            Box::new(PolicyRequest::Debug(protocol(s), true)),
        )),
        (_, Some(s @ "debug off"), None)
        | (_, Some(s @ "http debug off"), None)
        | (_, Some(s @ "tcp debug off"), None) => master.do_send(MasterCommand::UpdatePolicy(
            instance,
            Box::new(PolicyRequest::Debug(protocol(s), false)),
        )),
        (_, Some("http timeout"), Some(secs)) => {
            if let Ok(secs) = secs.parse::<u8>() {
                master.do_send(MasterCommand::UpdatePolicy(
                    instance,
                    Box::new(PolicyRequest::Timeout(secs)),
                ))
            } else {
                log::warn!("http timeout <seconds>: expecting u8, got {}", secs);
            }
        }
        (_, Some(s @ "policy"), Some(file))
        | (_, Some(s @ "http policy"), Some(file))
        | (_, Some(s @ "tcp policy"), Some(file)) => {
            let path = pathbuf(file);
            let protocol = protocol(s);
            match lang::Module::from_file(&path, Some(policy(&protocol))) {
                Ok(module) => {
                    let prog = module.program;
                    log::info!(
                        "sending {} policy: {}",
                        protocol,
                        prog.blake3_hash().unwrap()
                    );
                    master.do_send(MasterCommand::UpdatePolicy(
                        instance,
                        Box::new(PolicyRequest::SetPolicy(protocol, prog)),
                    ))
                }
                Err(err) => log::warn!(r#"{:?}: {}"#, path, err),
            }
        }
        (_, Some(s @ "allow all"), None)
        | (_, Some(s @ "http allow all"), None)
        | (_, Some(s @ "tcp allow all"), None)
        | (_, Some(s @ "deny all"), None)
        | (_, Some(s @ "http deny all"), None)
        | (_, Some(s @ "tcp deny all"), None) => {
            let protocol = protocol(s);
            let allow = s.contains("allow");
            if protocol == Protocol::All || protocol == Protocol::REST {
                set_policy(master, instance.clone(), Protocol::REST, allow)
            }
            if protocol == Protocol::All || protocol == Protocol::TCP {
                set_policy(master, instance, Protocol::TCP, allow)
            }
        }
        _ => log::info!("unknown command"),
    }
    false
}

pub fn run_command(
    master: &Addr<ArmourDataMaster>,
    cmd: &str,
    socket: &std::path::PathBuf,
) -> bool {
    let cmd = cmd.trim();
    if cmd != "" {
        if let Some(caps) = COMMAND.captures(&cmd) {
            return master_command(&master, caps, socket);
        } else {
            log::info!("unknown command <none>")
        }
    }
    false
}

pub fn run_script(script: &str, master: &Addr<ArmourDataMaster>, socket: &std::path::PathBuf) {
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
                        if run_command(master, &cmd, socket) {
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

fn is_rest(s: &str) -> bool {
    s.starts_with("http")
}

fn is_tcp(s: &str) -> bool {
    s.starts_with("tcp")
}

fn protocol(s: &str) -> Protocol {
    if is_rest(s) {
        Protocol::REST
    } else if is_tcp(s) {
        Protocol::TCP
    } else {
        Protocol::All
    }
}

fn policy(p: &Protocol) -> &lang::Interface {
    match p {
        Protocol::REST => &*lang::REST_POLICY,
        Protocol::TCP => &*lang::TCP_POLICY,
        Protocol::All => &*lang::TCP_REST_POLICY,
    }
}

fn armour_proxy() -> std::path::PathBuf {
    if let Ok(Some(path)) =
        std::env::current_exe().map(|path| path.parent().map(|dir| dir.join("armour-proxy")))
    {
        path
    } else {
        std::path::PathBuf::from("./armour-proxy")
    }
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

fn set_policy(
    master: &Addr<ArmourDataMaster>,
    instance: InstanceSelector,
    protocol: Protocol,
    allow: bool,
) {
    let policy = policy(&protocol);
    let module = if allow {
        lang::Module::allow_all(policy)
    } else {
        lang::Module::deny_all(policy)
    };
    let prog = module.unwrap().program;
    log::info!(
        r#"sending {} "{} all" policy: {}"#,
        protocol,
        if allow { "allow" } else { "deny" },
        prog.blake3_hash().unwrap()
    );
    master.do_send(MasterCommand::UpdatePolicy(
        instance,
        Box::new(PolicyRequest::SetPolicy(protocol, prog)),
    ))
}
