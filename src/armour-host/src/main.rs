//! Data Plane Host
//!
//! Controls proxy (data plane) instances and issues commands to them.
use actix::prelude::*;
use actix_web::{http, middleware, web, App, HttpServer};
use armour_host::{
    commands::{run_command, run_script},
    control_plane,
    host::{ArmourDataHost, Quit, UdsConnect},
    rest_api,
};
use armour_utils::parse_http_url;
use clap::{crate_version, App as ClapApp, Arg};
use futures::StreamExt;
use rustyline::{completion, error::ReadlineError, hint, validate::Validator, Editor};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Command Line Interface
    let matches = ClapApp::new("armour-host")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour Data Plane Host")
        .arg(
            Arg::with_name("control")
                .short("c")
                .long("control")
                .takes_value(true)
                .value_name("URL")
                .help("Control plane URL"),
        )
        .arg(
            Arg::with_name("url")
                .short("u")
                .long("url")
                .takes_value(true)
                .value_name("URL")
                .help("Data plane host URL (sent to control plane)"),
        )
        .arg(
            Arg::with_name("label")
                .long("label")
                .required(false)
                .takes_value(true)
                .help("Name of Armour host"),
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
                .help("Port for HTTP interface"),
        )
        .arg(
            Arg::with_name("host socket")
                .index(1)
                .required(false)
                .help("Unix socket of data plane host"),
        )
        .get_matches();

    // enable logging
    env::set_var("RUST_LOG", "armour_host=debug,armour_lang=debug,actix=info");
    env::set_var("RUST_BACKTRACE", "1");
    pretty_env_logger::init();

    // start Actix system
    let mut sys = actix_rt::System::new("armour_host");

    // get Armour password
    let pass = env::var("ARMOUR_PASS").unwrap_or_else(|_| {
        rpassword::read_password_from_tty(Some("password: ")).expect("failed to get password")
    });
    let pass_key = argon2rs::argon2i_simple(&pass, PASS_SALT);

    let control_url = parse_http_url(
        matches
            .value_of("control")
            .unwrap_or(armour_api::control::CONTROL_PLANE),
        8088,
    )?;

    // Unix socket for proxy communication
    let unix_socket = matches
        .value_of("host socket")
        .unwrap_or(armour_api::host::UDS_SOCKET)
        .to_string();
    let unix_socket = std::fs::canonicalize(&unix_socket)
        .unwrap_or_else(|_| std::path::PathBuf::from(unix_socket));

    // TCP socket for REST interface
    let port = matches
        .value_of("port")
        .map(|s| s.parse::<u16>().unwrap_or(armour_api::host::TCP_PORT))
        .unwrap_or(armour_api::host::TCP_PORT);
    let tcp_socket = std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(0, 0, 0, 0), port);

    // Onboarding data
    let label: armour_lang::labels::Label = matches
        .value_of("label")
        .unwrap_or("host")
        .parse()
        .map_err(|_| "host name must be a valid label")?;
    if label.len() != 1 {
        return Err("host label not of the form `<name>`".into());
    }
    let host = parse_http_url(
        matches
            .value_of("url")
            .unwrap_or(armour_api::host::DATA_PLANE_HOST),
        8090,
    )?;
    let onboard = armour_api::control::OnboardHostRequest {
        host,
        label: label.clone(),
        credentials: String::new(),
    };
    let onboard_clone = onboard.clone();

    // onboard with control plane
    let control_url_clone = control_url.clone();
    let onboarded = if let Err(message) = sys.block_on(async move {
        control_plane(
            &control_url_clone,
            http::Method::POST,
            "host/on-board",
            &onboard,
        )
        .await
    }) {
        log::warn!("failed to on-board with control plane: {}", message);
        false
    } else {
        log::info!("on-boarded with control plane");
        true
    };

    // start host actor, listening for connections on a Unix socket
    let unix_socket_clone = unix_socket.clone();
    let listener =
        Box::new(sys.block_on(async { tokio::net::UnixListener::bind(unix_socket_clone) })?);
    log::info!("started Data Host on socket: {}", unix_socket.display());
    let host = ArmourDataHost::create(|ctx| {
        ctx.add_message_stream(
            Box::leak(listener)
                .incoming()
                .map(|st| UdsConnect(st.unwrap())),
        );
        ArmourDataHost::new(&control_url, &label, onboarded, unix_socket, pass_key)
    });
    let host_clone = host.clone();

    // REST interface
    HttpServer::new(move || {
        App::new()
            .data(label.clone())
            .data(host_clone.clone())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/service")
                    .service(rest_api::service::on_board)
                    .service(rest_api::service::drop),
            )
            .service(
                web::scope("/host")
                    .service(rest_api::host::label)
                    .service(rest_api::host::proxies),
            )
            .service(
                web::scope("/policy")
                    .service(rest_api::policy::query)
                    .service(rest_api::policy::update),
            )
    })
    .bind(tcp_socket)?
    .run();

    // Interactive shell interface
    std::thread::spawn(move || {
        if let Some(script) = matches.value_of("script") {
            run_script(&host, script)
        };
        let mut rl = Editor::new();
        rl.set_helper(Some(Helper::new()));
        if rl.load_history("armour-host.txt").is_err() {
            log::info!("no previous history");
        }
        loop {
            match rl.readline("armour-host:> ") {
                Ok(cmd) => {
                    rl.add_history_entry(cmd.as_str());
                    if run_command(&host, &cmd) {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    host.do_send(Quit);
                    break;
                }
                Err(err) => log::warn!("{}", err),
            }
        }
        rl.save_history("armour-host.txt")
            .expect("failed to save history")
    });

    sys.run()?;

    if onboarded {
        // start new Actix system for sending a "drop-host" message to control plane
        let mut sys = actix_rt::System::new("armour_host");
        if let Err(message) = sys.block_on(async move {
            control_plane(
                &control_url,
                http::Method::DELETE,
                "host/drop",
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

const PASS_SALT: &str = "armour-host-salt";

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
