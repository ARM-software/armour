//! Data Plane Master
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use arm_policy::lang;
use armour_data_interface::{PolicyRequest, POLICY_SIG};
use armour_data_master as master;
use clap::{crate_version, App, Arg};
use master::{commands, MasterCommand};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
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
    env_logger::init();

    // start Actix system
    let sys = actix::System::new("armour-data-master");

    // start master actor
    let master = master::ArmourDataMaster::start_default();

    // start server, listening for connections on a Unix socket
    let socket = matches
        .value_of("master socket")
        .unwrap_or(SOCKET)
        .to_string();
    log::info!("starting Data Master on socket: {}", socket);
    let listener = tokio_uds::UnixListener::bind(socket.clone())?;
    let master_clone = master.clone();
    let _server = master::ArmourDataServer::create(|ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(master::UdsConnect));
        master::ArmourDataServer {
            master: master_clone,
            socket,
        }
    });

    // issue commands based on user input
    std::thread::spawn(move || loop {
        let mut cmd = String::new();
        if io::stdin().read_line(&mut cmd).is_err() {
            println!("error reading command");
            return;
        }
        if let Some(caps) = commands::MASTER.captures(&cmd) {
            master_command(&master, caps)
        } else if let Some(caps) = commands::INSTANCE0.captures(&cmd) {
            instance0_command(&master, caps)
        } else if let Some(caps) = commands::INSTANCE2.captures(&cmd) {
            instance2_command(&master, caps)
        } else if let Some(caps) = commands::INSTANCE1.captures(&cmd) {
            instance1_command(&master, caps)
        } else {
            log::info!("unknown command <none>")
        }
    });

    // handle Control-C
    let ctrl_c = tokio_signal::ctrl_c().flatten_stream();
    let handle_shutdown = ctrl_c
        .for_each(|()| {
            println!("Ctrl-C received, shutting down");
            System::current().stop();
            Ok(())
        })
        .map_err(|_| ());
    actix::spawn(handle_shutdown);

    sys.run()
}

fn master_command(master: &Addr<master::ArmourDataMaster>, caps: regex::Captures) {
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    match command.as_ref().map(String::as_str) {
        Some("list") => master.do_send(MasterCommand::ListActive),
        Some("quit") => master.do_send(MasterCommand::Quit),
        Some("help") => println!(
            "COMMANDS:
    help                      list commands
    list                      list connected instances
    quit                      shutdown master and all instances

    [<id>|all] allow all      request allow all policy
    [<id>|all] deny all       request deny all policy
    [<id>|all] ports          list active ports
    [<id>|all] shutdown       request shutdown
    [<id>|all] stop all       stop listening on all ports
    [<id>|all] policy <path>  read and request policy from file <path>
    [<id>|all] remote <path>  request read of policy from file <path>
    [<id>|all] start <port>   start listening for HTTP requests on port <port>
    [<id>|all] start tcp <port> <socket>
                              start listening on port <port> and forward
                              to <socket>
    [<id>|all] stop <port>    stop listening on port <port>

    <id>  single instance ID number
    all   all instances"
        ),
        _ => log::info!("unknown command"),
    }
}

fn instance0_command(master: &Addr<master::ArmourDataMaster>, caps: regex::Captures) {
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    if let Some(request) = match command.as_ref().map(String::as_str) {
        Some("ports") => Some(PolicyRequest::QueryActivePorts),
        Some("allow all") => Some(PolicyRequest::AllowAll),
        Some("deny all") => Some(PolicyRequest::DenyAll),
        Some("shutdown") => Some(PolicyRequest::Shutdown),
        Some("stop all") => Some(PolicyRequest::StopAll),
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

fn instance1_command(master: &Addr<master::ArmourDataMaster>, caps: regex::Captures) {
    let arg = caps.name("arg").unwrap().as_str().trim_matches('"');
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    if let Some(request) = match command.as_ref().map(String::as_str) {
        Some(s @ "start") | Some(s @ "stop") => {
            if let Ok(port) = arg.parse::<u16>() {
                Some(if s == "start" {
                    PolicyRequest::Start(port)
                } else {
                    PolicyRequest::Stop(port)
                })
            } else {
                log::warn!("{}: expecting port number, got {}", s, arg);
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

fn instance2_command(master: &Addr<master::ArmourDataMaster>, caps: regex::Captures) {
    let arg = caps.name("arg").unwrap().as_str();
    let port = caps.name("port").unwrap().as_str();
    let command = caps.name("command").map(|s| s.as_str().to_lowercase());
    if let Some(request) = match (port.parse::<u16>(), command.as_ref().map(String::as_str)) {
        (Ok(port), Some("start tcp")) => {
            if let Ok(sockets) = arg
                .to_socket_addrs()
                .map(|i| i.collect::<Vec<SocketAddr>>())
            {
                let sockets = sockets
                    .into_iter()
                    .filter(|s| s.is_ipv4())
                    .collect::<Vec<SocketAddr>>();
                match sockets.as_slice() {
                    [] => {
                        log::warn!("no socket");
                        None
                    }
                    [socket] => Some(PolicyRequest::StartTcp(port, *socket)),
                    _ => {
                        log::warn!("more than one socket: {:?}", sockets);
                        None
                    }
                }
            } else {
                log::warn!("could not parse socket");
                None
            }
        }
        (Err(_), Some("start tcp")) => {
            log::info!("bad port");
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
}
