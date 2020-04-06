use armour_proxy::{http_proxy, policy::PolicyActor};
use clap::{crate_version, App as ClapApp, Arg};
use std::convert::TryInto;
use std::env;

fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("armour-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Armour Proxy, with support for Security Policies")
        .arg(
            Arg::with_name("master socket")
                .index(1)
                .required(true)
                .help("Unix socket of data plane master"),
        )
        .arg(
            Arg::with_name("label")
                .long("label")
                .takes_value(true)
                .required(false)
                .help("label for proxy instance"),
        )
        .arg(
            Arg::with_name("log level")
                .short("l")
                .takes_value(true)
                .possible_values(&["error", "warn", "info", "debug", "trace"])
                .help("log level"),
        )
        .get_matches();

    let log_level = matches.value_of("log level").unwrap_or("debug");

    // enable logging
    env::set_var(
        "RUST_LOG",
        format!(
            "armour_proxy={l},armour_lang={l},actix_web={l}",
            l = log_level
        ),
    );
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // get Armour key
    let key: [u8; 32] = base64::decode(
        env::var("ARMOUR_PASS")
            .expect("ARMOUR_PASS environment variable not set")
            .as_str(),
    )
    .expect("ARMOUR_PASS is not base64 encoded")
    .as_slice()
    .try_into()
    .expect("ARMOUR_PASS is wrong length");
    // let key = [0; 32];

    // start Actix system
    let mut sys = actix_rt::System::new("armour_proxy");

    log::info!("local host names are: {:?}", *http_proxy::LOCAL_HOST_NAMES);

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)
    // install the CLI policy
    let master_socket = matches.value_of("master socket").unwrap().to_string();
    log::info!("connecting to: {}", master_socket);
    let stream = sys.block_on(tokio::net::UnixStream::connect(master_socket))?;
    match matches.value_of("label").unwrap_or("proxy").parse() {
        Ok(label) => {
            PolicyActor::create_policy(stream, label, key);
            sys.run()
        }
        Err(err) => {
            log::warn!("{}", err);
            Ok(())
        }
    }
}
