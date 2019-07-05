//! Data Plane Master
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use arm_policy::lang;
use armour_data_interface::PolicyRequest;
use armour_data_master as master;
use clap::{crate_version, App, Arg};
use master::{commands, MasterCommand};
use std::io;
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
        ctx.add_message_stream(
            listener
                .incoming()
                .map_err(|_| ())
                .map(|st| master::UdsConnect(st)),
        );
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
            let command = caps.name("command").map(|s| s.as_str().to_lowercase());
            match command.as_ref().map(String::as_str) {
                Some("list") => master.do_send(MasterCommand::ListActive),
                Some("help") => println!(
                    "COMMANDS:
    help                      list commands
    list                      list connected instances

    [<id>|all] allow all      request allow all policy
    [<id>|all] deny all       request deny all policy
    [<id>|all] shutdown       request shutdown
    [<id>|all] policy <path>  read and request policy from file <path>
    [<id>|all] remote <path>  request read of policy from file <path>

    <id>  single instance ID number
    all   all instances"
                ),
                _ => log::info!("unknown command"),
            }
        } else if let Some(caps) = commands::INSTANCE0.captures(&cmd) {
            let command = caps.name("command").map(|s| s.as_str().to_lowercase());
            if let Some(request) = match command.as_ref().map(String::as_str) {
                Some("allow all") => Some(PolicyRequest::AllowAll),
                Some("deny all") => Some(PolicyRequest::DenyAll),
                Some("shutdown") => Some(PolicyRequest::Shutdown),
                _ => {
                    log::info!("unknown command");
                    None
                }
            } {
                master.do_send(MasterCommand::UpdatePolicy(
                    commands::instance(&caps),
                    request,
                ))
            }
        } else if let Some(caps) = commands::INSTANCE1.captures(&cmd) {
            let path = PathBuf::from(caps.name("path").unwrap().as_str());
            let command = caps.name("command").map(|s| s.as_str().to_lowercase());
            if let Some(request) = match command.as_ref().map(String::as_str) {
                Some("policy") => match lang::Program::from_file(&path) {
                    Ok(prog) => Some(PolicyRequest::UpdateFromData(prog)),
                    Err(err) => {
                        log::warn!(r#"{:?}: {}"#, path, err);
                        None
                    }
                },
                Some("remote") => Some(PolicyRequest::UpdateFromFile(path)),
                _ => {
                    log::info!("unknown command");
                    None
                }
            } {
                master.do_send(MasterCommand::UpdatePolicy(
                    commands::instance(&caps),
                    request,
                ))
            }
        } else {
            log::info!("unknown command")
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
