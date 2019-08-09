//! Data Plane Master
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use armour_policy::lang;
use armour_data_interface::{PolicyRequest, ProxyConfig, POLICY_SIG};
use armour_data_master as master;
use clap::{crate_version, App, Arg};
use master::{commands, MasterCommand};
use rustyline::{completion, error::ReadlineError, hint, Editor};
use std::io::{self, BufRead};
use std::path::PathBuf;

fn main() -> io::Result<()> {
    const SOCKET: &str = "armour";

    // CLI
    let matches = App::new("armour-data-master")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Data Plane Master")
        .arg(
            Arg::with_name("master socket")
                .index(1)
                .required(false)
                .help("Unix socket of data plane master"),
        )
        .get_matches();

    // enable logging
    std::env::set_var("RUST_LOG", "armour_data_master=debug,actix=debug");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // start Actix system
    let sys = actix::System::new("armour-data-master");

    // start master actor
    let master = master::ArmourDataMaster::start_default();

    // start server, listening for connections on a Unix socket
    let socket = matches
        .value_of("master socket")
        .unwrap_or(SOCKET)
        .to_string();
    let listener = tokio_uds::UnixListener::bind(&socket)?;
    let socket =
        std::fs::canonicalize(&socket).unwrap_or_else(|_| std::path::PathBuf::from(socket));
    log::info!("started Data Master on socket: {}", socket.display());
    let master_clone = master.clone();
    let socket_clone = socket.clone();
    let _server = master::ArmourDataServer::create(|ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(master::UdsConnect));
        master::ArmourDataServer {
            master: master_clone,
            socket: socket_clone,
        }
    });

    // issue commands based on user input
    std::thread::spawn(move || {
        let mut rl = Editor::new();
        rl.set_helper(Some(Helper::new()));
        if rl.load_history("armour-master-history.txt").is_err() {
            log::info!("no previous history");
        }
        loop {
            match rl.readline("armour-master:> ") {
                Ok(cmd) => {
                    rl.add_history_entry(cmd.as_str());
                    if run_command(&master, &cmd, &socket) {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    master.do_send(MasterCommand::Quit);
                    break;
                }
                Err(err) => log::warn!("{}", err),
            }
        }
        rl.save_history("armour-master-history.txt")
            .expect("failed to save history")
    });

    sys.run()
}

struct Helper(completion::FilenameCompleter, hint::HistoryHinter);

impl Helper {
    fn new() -> Self {
        Helper(completion::FilenameCompleter::new(), hint::HistoryHinter {})
    }
}

impl completion::Completer for Helper {
    type Candidate = completion::Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> Result<(usize, Vec<completion::Pair>), ReadlineError> {
        self.0.complete(line, pos, ctx)
    }
}
impl hint::Hinter for Helper {
    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<String> {
        self.1.hint(line, pos, ctx)
    }
}

impl rustyline::highlight::Highlighter for Helper {}

impl rustyline::Helper for Helper {}

fn armour_data() -> std::path::PathBuf {
    if let Ok(Some(path)) =
        std::env::current_exe().map(|path| path.parent().map(|dir| dir.join("armour-data")))
    {
        path
    } else {
        std::path::PathBuf::from("./armour-data")
    }
}

fn run_command(
    master: &Addr<master::ArmourDataMaster>,
    cmd: &str,
    socket: &std::path::PathBuf,
) -> bool {
    let cmd = cmd.trim();
    if cmd != "" {
        if let Some(caps) = commands::MASTER.captures(&cmd) {
            return master_command(&master, caps, socket);
        } else if let Some(caps) = commands::INSTANCE0.captures(&cmd) {
            instance0_command(&master, caps)
        } else if let Some(caps) = commands::INSTANCE1.captures(&cmd) {
            return instance1_command(&master, caps, socket);
        } else {
            log::info!("unknown command <none>")
        }
    }
    false
}

fn master_command(
    master: &Addr<master::ArmourDataMaster>,
    caps: regex::Captures,
    socket: &std::path::PathBuf,
) -> bool {
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    match command.as_ref().map(String::as_str) {
        Some("list") => master.do_send(MasterCommand::ListActive),
        Some("quit") => {
            master.do_send(MasterCommand::Quit);
            return true;
        }
        Some(s @ "launch log") | Some(s @ "launch") => {
            let log = if s.ends_with("log") {
                "-l debug"
            } else {
                "-l error"
            };
            match std::process::Command::new(armour_data())
                .arg(log)
                .arg(socket)
                .spawn()
            {
                Ok(child) => log::info!("started processs: {}", child.id()),
                Err(err) => log::warn!("failed to spawn data plane instance: {}", err),
            }
        }
        Some("help") => println!(
            "COMMANDS:
    help                      list commands
    launch [log]              start a new slave instance
    list                      list connected instances
    quit                      shutdown master and all instances

    run <file>                run commands from <file>
    wait <seconds>            wait for <seconds> to elapse (up to 10s)

    [<id>|all] allow all      request allow all policy
    [<id>|all] deny all       request deny all policy
    [<id>|all] debug on|off   enable/disable display of HTTP requests
    [<id>|all] ports          list active ports
    [<id>|all] shutdown       request shutdown
    [<id>|all] timeout <secs> server response timeout
    [<id>|all] policy <path>  read policy from <path> and send to instance
    [<id>|all] remote <path>  ask instance to read policy from <path>
    [<id>|all] stop all       stop listening on all ports
    [<id>|all] stop <port>    stop listening on <port>
    [<id>|all] start [tcp|streaming] <port>
                              start listening for HTTP/TCP requests on <port>

    <id>  instance ID number
    all   all instances"
        ),
        _ => log::info!("unknown command"),
    }
    false
}

fn instance0_command(master: &Addr<master::ArmourDataMaster>, caps: regex::Captures) {
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    if let Some(request) = match command.as_ref().map(String::as_str) {
        Some("ports") => Some(PolicyRequest::QueryActivePorts),
        Some("allow all") => Some(PolicyRequest::AllowAll),
        Some("deny all") => Some(PolicyRequest::DenyAll),
        Some("shutdown") => Some(PolicyRequest::Shutdown),
        Some("stop all") => Some(PolicyRequest::StopAll),
        Some("debug on") => Some(PolicyRequest::Debug(true)),
        Some("debug off") => Some(PolicyRequest::Debug(false)),
        _ => {
            log::info!("unknown command");
            None
        }
    } {
        master.do_send(MasterCommand::UpdatePolicy(
            commands::instance(&caps),
            Box::new(request),
        ))
    }
}

#[allow(clippy::cognitive_complexity)]
fn instance1_command(
    master: &Addr<master::ArmourDataMaster>,
    caps: regex::Captures,
    socket: &std::path::PathBuf,
) -> bool {
    let arg = caps.name("arg").unwrap().as_str().trim_matches('"');
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    if let Some(request) = match command.as_ref().map(String::as_str) {
        Some("start tcp") => {
            if let Ok(port) = arg.parse::<u16>() {
                Some(PolicyRequest::StartTcp(port))
            } else {
                log::warn!("start tcp: expecting port number, got {}", arg);
                None
            }
        }
        Some(s @ "start") | Some(s @ "start streaming") | Some(s @ "stop") => {
            if let Ok(port) = arg.parse::<u16>() {
                Some(if s.starts_with("start") {
                    let streaming = s.ends_with("streaming");
                    PolicyRequest::Start(ProxyConfig {
                        port,
                        request_streaming: streaming,
                        response_streaming: streaming,
                    })
                } else {
                    PolicyRequest::Stop(port)
                })
            } else {
                log::warn!("{}: expecting port number, got {}", s, arg);
                None
            }
        }
        Some("timeout") => {
            if let Ok(secs) = arg.parse::<u8>() {
                Some(PolicyRequest::Timeout(secs))
            } else {
                log::warn!("expecting timeout in seconds, got {}", arg);
                None
            }
        }
        Some("policy") => {
            let path = PathBuf::from(arg);
            match lang::Program::check_from_file(&path, &*POLICY_SIG) {
                Ok(prog) => Some(PolicyRequest::UpdateFromData(prog)),
                Err(err) => {
                    log::warn!(r#"{:?}: {}"#, path, err);
                    None
                }
            }
        }
        Some("remote") => Some(PolicyRequest::UpdateFromFile(PathBuf::from(arg))),
        Some("wait") => {
            if commands::instance(&caps) == master::Instances::SoleInstance {
                if let Ok(delay) = arg.parse::<u8>() {
                    std::thread::sleep(std::time::Duration::from_secs(delay.min(10).into()))
                } else {
                    log::warn!("expecting u8, got {}", arg);
                }
            } else {
                log::info!("unknown command")
            };
            None
        }
        Some("run") => {
            if commands::instance(&caps) == master::Instances::SoleInstance {
                let arg = arg.replace("\\ ", " ");
                match std::fs::File::open(PathBuf::from(&arg)) {
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
                                        return true;
                                    }
                                };
                                line += 1;
                                done = res == 0
                            } else {
                                log::warn!("{}: error reading command on line {}", arg, line);
                                done = true
                            }
                        }
                    }
                    Err(err) => log::warn!("{}: {}", arg, err),
                }
            } else {
                log::info!("unknown command")
            };
            None
        }
        _ => {
            log::info!("unknown command");
            None
        }
    } {
        master.do_send(MasterCommand::UpdatePolicy(
            commands::instance(&caps),
            Box::new(request),
        ))
    }
    false
}
