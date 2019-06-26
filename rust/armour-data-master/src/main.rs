#[macro_use]
extern crate lazy_static;

use actix::prelude::*;
use armour_data_master as master;
use clap::{crate_version, App, Arg};
use master::MasterCommand;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    const SOCKET: &str = "armour";

    // CLI
    let matches = App::new("armour-data-master")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Proxy, with support for Security Policies")
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
    let sys = actix::System::new("armour-data");

    // start up master actor
    let master = master::ArmourDataMaster::start_default();

    // start up server on Unix socket
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

    // check for user input - send a hello
    std::thread::spawn(move || loop {
        let mut cmd = String::new();
        if io::stdin().read_line(&mut cmd).is_err() {
            println!("error");
            return;
        }
        if commands::LIST.is_match(&cmd) {
            master.do_send(MasterCommand::ListActive)
        } else if let Some(caps) = commands::LOAD.captures(&cmd) {
            if let Ok(instance) = caps.name("instance").unwrap().as_str().parse::<usize>() {
                let path = caps.name("path").unwrap().as_str();
                master.do_send(MasterCommand::PolicyFile(instance, PathBuf::from(path)))
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

mod commands {
    use regex::Regex;

    // commands:
    // - list
    // - load <n> <path>

    lazy_static! {
        pub static ref LIST: Regex = Regex::new(r"^(?i)\s*list\s*$").unwrap();
    }

    lazy_static! {
        pub static ref LOAD: Regex =
            Regex::new(r"^(?i)\s*load\s*(?P<instance>[[:digit:]]+)\s*(?P<path>\S*)\s*$").unwrap();
    }
}