//! Data Plane Master
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use actix_web::{http, middleware, web, App, HttpServer};
use armour_master::{
    commands::{run_command, run_script},
    control_plane,
    master::{ArmourDataMaster, Quit, UdsConnect},
    rest_api,
};
use clap::{crate_version, App as ClapApp, Arg};
use futures::StreamExt;
use rustyline::{completion, error::ReadlineError, hint, validate::Validator, Editor};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const UDS_SOCKET: &str = "armour";
    const TCP_PORT: u16 = 8090;

    // Command Line Interface
    let matches = ClapApp::new("armour-master")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Data Plane Master")
        .arg(
            Arg::with_name("label")
                .long("label")
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
            Arg::with_name("port")
                .short("p")
                .required(false)
                .takes_value(true)
                .help("TCP port for REST interface"),
        )
        .arg(
            Arg::with_name("master socket")
                .index(1)
                .required(false)
                .help("Unix socket of data plane master"),
        )
        .get_matches();

    // enable logging
    env::set_var(
        "RUST_LOG",
        "armour_master=debug,armour_lang=debug,actix=info",
    );
    env::set_var("RUST_BACKTRACE", "1");
    pretty_env_logger::init();

    // start Actix system
    let mut sys = actix_rt::System::new("armour_master");

    // get Armour password
    let pass = env::var("ARMOUR_PASS").unwrap_or_else(|_| {
        rpassword::read_password_from_tty(Some("password: ")).expect("failed to get password")
    });
    let pass_key = argon2rs::argon2i_simple(&pass, PASS_SALT);

    // Unix socket for proxy communication
    let unix_socket = matches
        .value_of("master socket")
        .unwrap_or(UDS_SOCKET)
        .to_string();
    let unix_socket = std::fs::canonicalize(&unix_socket)
        .unwrap_or_else(|_| std::path::PathBuf::from(unix_socket));

    // TCP socket for REST interface
    let port = matches
        .value_of("port")
        .map(|s| s.parse::<u16>().unwrap_or(TCP_PORT))
        .unwrap_or(TCP_PORT);
    let tcp_socket = format!("localhost:{}", port);

    // Onboarding data
    let label: armour_lang::labels::Label = matches
        .value_of("label")
        .unwrap_or("master")
        .parse()
        .map_err(|_| "master name must be a valid label")?;
    if label.len() != 1 {
        return Err("master label not of the form `<name>`".into());
    }
    let onboard = armour_api::control::OnboardMasterRequest {
        host: url::Url::parse(&tcp_socket).unwrap(), // TODO: public URL from command line
        master: label.clone(),
        credentials: String::new(),
    };
    let onboard_clone = onboard.clone();

    // start master actor, listening for connections on a Unix socket
    let unix_socket_clone = unix_socket.clone();
    let listener =
        Box::new(sys.block_on(async { tokio::net::UnixListener::bind(unix_socket_clone) })?);
    log::info!("started Data Master on socket: {}", unix_socket.display());
    let master = ArmourDataMaster::create(|ctx| {
        ctx.add_message_stream(
            Box::leak(listener)
                .incoming()
                .map(|st| UdsConnect(st.unwrap())),
        );
        ArmourDataMaster::new(unix_socket, pass_key)
    });
    let master_clone = master.clone();

    // onboard with control plane
    let onboarded;
    if let Err(message) = sys.block_on(async move {
        control_plane(
            &actix_web::client::Client::default(),
            http::Method::POST,
            "on-board-master",
            &onboard,
        )
        .await
    }) {
        onboarded = false;
        log::warn!("failed to on-board with control plane: {}", message)
    } else {
        onboarded = true;
        log::info!("on-boarded with control plane")
    };

    // REST interface
    HttpServer::new(move || {
        App::new()
            .data(label.clone())
            .data(master_clone.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/launch")
                    .service(rest_api::launch::on_board_services)
                    .service(rest_api::launch::drop_services),
            )
            .service(
                web::scope("/master")
                    .service(rest_api::master::label)
                    .service(rest_api::master::proxies),
            )
            .service(
                web::scope("/policy")
                    .service(rest_api::policy::query)
                    .service(rest_api::policy::update),
            )
    })
    .bind(&tcp_socket)?
    .run();

    // Interactive shell interface
    std::thread::spawn(move || {
        if let Some(script) = matches.value_of("script") {
            run_script(&master, script)
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
                    if run_command(&master, &cmd) {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    master.do_send(Quit);
                    break;
                }
                Err(err) => log::warn!("{}", err),
            }
        }
        rl.save_history("armour-master.txt")
            .expect("failed to save history")
    });

    sys.run()?;

    if onboarded {
        // start new Actix system for sending a "drop-master" message to control plane
        let mut sys = actix_rt::System::new("armour_master");
        if let Err(message) = sys.block_on(async move {
            control_plane(
                &actix_web::client::Client::default(),
                http::Method::DELETE,
                "drop-master",
                &onboard_clone,
            )
            .await
        }) {
            log::warn!("failed to notify control plane: {}", message)
        } else {
            log::info!("notified control plane")
        }
    }

    Ok(())
}

const PASS_SALT: &str = "armour-master-salt";

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
