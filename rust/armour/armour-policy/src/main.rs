/// Armour policy language
use armour_policy::lang;
use clap::{crate_version, App, Arg};
use futures::{future, Future};
use rustyline::{error::ReadlineError, Editor};
use std::io;
use std::sync::Arc;
use std::time::Duration;

fn main() -> io::Result<()> {
    // command line interface
    let matches = App::new("Armour")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour policy language REPL")
        .arg(
            Arg::with_name("input file")
                .index(1)
                .required(false)
                .help("Policy file"),
        )
        .arg(
            Arg::with_name("timeout")
                .long("timeout")
                .takes_value(true)
                .help("Timeout (seconds) for external RPCs\n(default: 3s)"),
        )
        .get_matches();

    // enable logging
    std::env::set_var("RUST_LOG", "armour_policy=info,actix=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // declare program
    let mut prog: lang::Program;

    // try to load code from an input file
    if let Some(filename) = matches.value_of("input file") {
        prog = lang::Program::from_file(filename)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    } else {
        prog = lang::Program::default()
    }
    prog.print();

    if let Some(timeout) = matches.value_of("timeout") {
        let d = Duration::from_secs(timeout.parse().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "timeout (seconds) must be an integer",
            )
        })?);
        prog.set_timeout(d)
    }

    // test: serialize then deserialize program (using bincode)
    // let bytes = prog.to_bytes()?;
    // println!("{:?}", bytes);
    // prog = lang::Program::from_bytes(&bytes)?;

    let mut sys = actix::System::new("armour-policy");

    // evaluate expressions (REPL)
    let headers = prog.headers.clone();
    let prog = Arc::new(prog);
    let mut rl = Editor::<()>::new();
    if rl.load_history("armour-policy.txt").is_err() {
        log::info!("no previous history");
    }
    loop {
        match rl.readline("armour:> ") {
            Ok(line) => {
                let line = line.trim();
                if line != "" {
                    rl.add_history_entry(line);
                    match lang::Expr::from_string(line, &headers) {
                        Ok(e) => {
                            // println!("{:#?}", e);
                            let fut = e
                                .evaluate(prog.clone())
                                .and_then(|r| {
                                    r.print();
                                    // println!(": {}", r.clone());
                                    future::ok(())
                                })
                                .or_else(|err| {
                                    log::warn!("{}", err);
                                    future::ok(())
                                })
                                .map_err(|_: ()| (std::io::ErrorKind::Other));
                            sys.block_on(fut)?
                        }
                        Err(err) => log::warn!("{}", err),
                    }
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => log::warn!("{}", err),
        }
    }
    // done
    rl.save_history("armour-policy.txt")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}
