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
            println!("error");
            return;
        }
        if commands::LIST.is_match(&cmd) {
            master.do_send(MasterCommand::ListActive)
        } else if let Some(caps) = commands::POLICY.captures(&cmd) {
            let path = PathBuf::from(caps.name("path").unwrap().as_str());
            match lang::Program::from_file(&path).map(|prog| prog.to_bytes()) {
                Ok(Ok(bytes)) => master.do_send(MasterCommand::UpdatePolicy(
                    commands::instance(&caps),
                    PolicyRequest::UpdateFromData(bytes),
                )),
                Ok(Err(err)) => log::warn!(r#"{:?}: {}"#, path, err),
                Err(err) => log::warn!(r#"{:?}: {}"#, path, err),
            }
        } else if let Some(caps) = commands::ALLOW_ALL.captures(&cmd) {
            master.do_send(MasterCommand::UpdatePolicy(
                commands::instance(&caps),
                PolicyRequest::AllowAll,
            ))
        } else if let Some(caps) = commands::DENY_ALL.captures(&cmd) {
            master.do_send(MasterCommand::UpdatePolicy(
                commands::instance(&caps),
                PolicyRequest::DenyAll,
            ))
        } else if let Some(caps) = commands::REMOTE.captures(&cmd) {
            master.do_send(MasterCommand::UpdatePolicy(
                commands::instance(&caps),
                PolicyRequest::UpdateFromFile(PathBuf::from(caps.name("path").unwrap().as_str())),
            ))
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
