//! Data Plane Master
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use actix_web::{middleware, web, App, HttpServer};
use armour_master::{
    commands::{run_command, run_script},
    master::{ArmourDataMaster, MasterCommand, UdsConnect},
    rest_policy,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::StreamExt;
use rustyline::{completion, error::ReadlineError, hint, validate::Validator, Editor};
use std::io;

fn main() -> io::Result<()> {
    const UDS_SOCKET: &str = "armour";
    const TCP_SOCKET: &str = "127.0.0.1:8090";

    // Command Line Interface
    let matches = ClapApp::new("armour-master")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Data Plane Master")
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .required(false)
                .takes_value(true)
                .help("Name of Armour master"),
        )
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
        "armour_master=debug,armour_lang=debug,actix=info",
    );
    std::env::set_var("RUST_BACKTRACE", "1");
    pretty_env_logger::init();

    // start Actix system
    let mut sys = actix_rt::System::new("armour_master");

    // start master, listening for connections on a Unix socket
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
    let name = matches.value_of("name").unwrap_or("master").to_string();
    let master_clone = master.clone();
    HttpServer::new(move || {
        App::new()
            .data(name.clone())
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

// rustyline configuration

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
