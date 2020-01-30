/// Armour policy language
use actix::prelude::*;
use armour_lang::{expressions, interpret::Env, lang};
use clap::{crate_version, App, Arg, SubCommand};
use rustyline::{error::ReadlineError, Editor};
use std::io;
use std::sync::Arc;
use std::time::Duration;

struct Eval {
    env: Arc<Env>,
}

impl Eval {
    fn new(prog: lang::Program) -> Self {
        Eval {
            env: Arc::new(Env::new(Arc::new(prog))),
        }
    }
}

impl Actor for Eval {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<expressions::Expr, expressions::Error>")]
struct Evaluate(expressions::Expr);

impl Handler<Evaluate> for Eval {
    type Result = ResponseFuture<Result<expressions::Expr, expressions::Error>>;
    fn handle(&mut self, msg: Evaluate, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(msg.0.evaluate(self.env.clone()))
    }
}

struct Stop;

impl Message for Stop {
    type Result = ();
}

impl Handler<Stop> for Eval {
    type Result = ();
    fn handle(&mut self, _msg: Stop, _ctx: &mut Context<Self>) {
        System::current().stop()
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // command line interface
    let matches = App::new("armour-lang")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour policy language REPL")
        .arg(
            Arg::with_name("bincode")
                .long("bincode")
                .required(false)
                .takes_value(true)
                .requires("input file")
                .conflicts_with("protocol")
                .help("Load policy from bincode input"),
        )
        .arg(
            Arg::with_name("policy")
                .long("policy")
                .required(false)
                .takes_value(true)
                .conflicts_with("input file")
                .requires("protocol")
                .possible_values(&["allow", "deny"])
                .help("Type of policy"),
        )
        .arg(
            Arg::with_name("protocol")
                .long("protocol")
                .required(false)
                .takes_value(true)
                .possible_values(&["tcp", "http"])
                .help("Type of protocol"),
        )
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
        .subcommand(
            SubCommand::with_name("export")
                .about("serialise programs")
                .arg(
                    Arg::with_name("format")
                        .index(1)
                        .required(true)
                        .possible_values(&[
                            "armour",
                            "bincode",
                            "blake3",
                            "json",
                            "pretty-json",
                            "yaml",
                        ]),
                ),
        )
        .get_matches();

    // enable logging
    std::env::set_var("RUST_LOG", "armour_lang=info,actix=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // try to load code from an input file, or by using policy + protocol option
    let file = matches.value_of("input file");
    let mut prog = if matches.is_present("bincode") {
        lang::Program::from_bincode(file.unwrap())?
    } else {
        match (
            matches.value_of("policy"),
            matches.value_of("protocol"),
            file,
        ) {
            (Some("allow"), Some("tcp"), None) => lang::Program::allow_all(&*lang::TCP_POLICY)?,
            (Some("allow"), Some("http"), None) => lang::Program::allow_all(&*lang::REST_POLICY)?,
            (Some("deny"), Some("tcp"), None) => lang::Program::deny_all(&*lang::TCP_POLICY)?,
            (Some("deny"), Some("http"), None) => lang::Program::deny_all(&*lang::REST_POLICY)?,
            (None, Some("tcp"), Some(file)) => {
                lang::Program::from_file(file, Some(&*lang::TCP_POLICY))?
            }
            (None, Some("http"), Some(file)) => {
                lang::Program::from_file(file, Some(&*lang::REST_POLICY))?
            }
            (_, _, Some(file)) => lang::Program::from_file(file, None)?,
            _ => lang::Program::default(),
        }
    };
    if let Some(timeout) = matches.value_of("timeout") {
        let d = Duration::from_secs(timeout.parse().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "timeout (seconds) must be an integer",
            )
        })?);
        prog.set_timeout(d)
    }

    if let Some(matches) = matches.subcommand_matches("export") {
        let s = match matches.value_of("format").unwrap() {
            "armour" => prog.to_string(),
            "bincode" => prog.to_bincode().unwrap_or_else(|e| e.to_string()),
            "blake3" => prog
                .blake3_hash()
                .map(|a| a.to_string())
                .unwrap_or_else(|| "<failed to hash>".to_string()),
            "json" => serde_json::to_string(&prog).unwrap_or_else(|e| e.to_string()),
            "pretty-json" => serde_json::to_string_pretty(&prog).unwrap_or_else(|e| e.to_string()),
            "yaml" => serde_yaml::to_string(&prog).unwrap_or_else(|e| e.to_string()),
            _ => unreachable!(),
        };
        print!("{}", s)
    } else {
        // start eval actor
        let headers = prog.headers.clone();
        let eval = Eval::new(prog).start();

        // evaluate expressions (REPL)
        let mut rl = Editor::<()>::new();
        if rl.load_history(".armour-lang.txt").is_err() {
            log::info!("no previous history");
        };
        loop {
            match rl.readline("armour:> ") {
                Ok(line) => {
                    let line = line.trim();
                    if line != "" {
                        rl.add_history_entry(line);
                        match expressions::Expr::from_string(line, &headers) {
                            Ok(e) => {
                                // println!("{:#?}", e);
                                let now = std::time::Instant::now();
                                match eval.send(Evaluate(e)).await {
                                    Ok(Ok(r)) => {
                                        log::info!("eval time: {:?}", now.elapsed());
                                        r.print()
                                    }
                                    Ok(Err(e)) => log::warn!("{}", e),
                                    Err(_e) => (),
                                }
                            }
                            Err(err) => log::warn!("{}", err),
                        }
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    eval.do_send(Stop);
                    break;
                }
                Err(err) => log::warn!("{}", err),
            }
        }
        // done
        if let Err(e) = rl
            .save_history("armour-lang.txt")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        {
            log::warn!("{}", e)
        }
    }

    Ok(())
}
