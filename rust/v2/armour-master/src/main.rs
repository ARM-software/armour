//! Data Plane Master
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use actix_web::{middleware, web, App, HttpServer};
use armour_api::proxy::{PolicyRequest, Protocol};
use armour_lang::lang;
use armour_master::{
    commands, rest_policy, AddChild, ArmourDataMaster, InstanceSelector, MasterCommand, UdsConnect,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::StreamExt;
use rustyline::{completion, error::ReadlineError, hint, validate::Validator, Editor};
use std::io::{self, BufRead};
use std::path::PathBuf;

fn main() -> io::Result<()> {
    const UDS_SOCKET: &str = "armour";
    const TCP_SOCKET: &str = "127.0.0.1:8090";

    // Command Line Interface
    let matches = ClapApp::new("armour-master")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Data Plane Master")
        .arg(
            Arg::with_name("script")
                .short("r")
                .long("run")
                .required(false)
                .takes_value(true)
                .help("Run commands from a script"),
        )
        .arg(
            Arg::with_name("master socket")
                .index(1)
                .required(false)
                .help("Unix socket of data plane master"),
        )
        .get_matches();

    // enable logging
    std::env::set_var(
        "RUST_LOG",
        "armour_master=debug,armour_lang=debug,actix=debug",
    );
    std::env::set_var("RUST_BACKTRACE", "1");
    pretty_env_logger::init();

    // start Actix system
    let mut sys = actix_rt::System::new("armour_master");

    // start server, listening for connections on a Unix socket
    let socket = matches
        .value_of("master socket")
        .unwrap_or(UDS_SOCKET)
        .to_string();
    let socket_clone = socket.clone();
    let listener =
        Box::new(sys.block_on(async move { tokio::net::UnixListener::bind(socket_clone) })?);
    let socket =
        std::fs::canonicalize(&socket).unwrap_or_else(|_| std::path::PathBuf::from(socket));
    log::info!("started Data Master on socket: {}", socket.display());
    let socket_clone = socket.clone();
    let master = ArmourDataMaster::create(|ctx| {
        ctx.add_message_stream(
            Box::leak(listener)
                .incoming()
                .map(|st| UdsConnect(st.unwrap())),
        );
        ArmourDataMaster::new(socket_clone)
    });

    // REST interface
    let master_clone = master.clone();
    HttpServer::new(move || {
        App::new()
            .data(master_clone.clone())
            .wrap(middleware::Logger::default())
            .service(web::scope("/policy").service(rest_policy::update))
    })
    .bind(TCP_SOCKET)?
    .run();

    // Interactive shell interface
    std::thread::spawn(move || {
        if let Some(script) = matches.value_of("script") {
            run_script(script, &master, &socket)
        };
        let mut rl = Editor::new();
        rl.set_helper(Some(Helper::new()));
        if rl.load_history("armour-master.txt").is_err() {
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
        rl.save_history("armour-master.txt")
            .expect("failed to save history")
    });

    sys.run()
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

fn run_script(script: &str, master: &Addr<ArmourDataMaster>, socket: &std::path::PathBuf) {
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

fn run_command(master: &Addr<ArmourDataMaster>, cmd: &str, socket: &std::path::PathBuf) -> bool {
    let cmd = cmd.trim();
    if cmd != "" {
        if let Some(caps) = commands::COMMAND.captures(&cmd) {
            return master_command(&master, caps, socket);
        } else {
            log::info!("unknown command <none>")
        }
    }
    false
}

fn master_command(
    master: &Addr<ArmourDataMaster>,
    caps: regex::Captures,
    socket: &std::path::PathBuf,
) -> bool {
    let (instance, command, args) = commands::command(&caps);
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
        (true, Some(s @ "launch log"), None) | (true, Some(s @ "launch"), None) => {
            let log = if s.ends_with("log") {
                "-l info"
            } else {
                "-l error"
            };
            let armour_proxy = armour_proxy();
            log::info!("launching: {}", armour_proxy.display());
            // sudo ~/.cargo/bin/flamegraph ~/rust/target/debug/armour-data
            match std::process::Command::new(armour_proxy)
                .arg(log)
                .arg(socket)
                .spawn()
            {
                Ok(child) => {
                    let pid = child.id();
                    master.do_send(AddChild(pid, child));
                    log::info!("started processs: {}", pid)
                }
                Err(err) => log::warn!("failed to spawn data plane instance: {}", err),
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

fn policy(p: &Protocol) -> &lang::Interface {
    match p {
        Protocol::REST => &*lang::REST_POLICY,
        Protocol::TCP => &*lang::TCP_POLICY,
        Protocol::All => &*lang::TCP_REST_POLICY,
    }
}

struct Helper(completion::FilenameCompleter, hint::HistoryHinter);

impl Validator for Helper {}

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
