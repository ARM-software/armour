/// Armour policy language
use actix::prelude::*;
use armour_lang::{expressions, interpret::Env, lang, policies};
use clap::{crate_version, App, Arg, SubCommand};
use rustyline::{error::ReadlineError, Editor};
use std::io;
use std::time::Duration;

struct Eval {
    env: Env,
}

impl Eval {
    fn new(prog: &lang::Program) -> Self {
        Eval {
            env: Env::new(prog),
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
            Arg::with_name("input file")
                .index(1)
                .required(false)
                .help("Policy file"),
        )
        .subcommand(
            SubCommand::with_name("export")
                .about("Serialise policy")
                .arg(
                    Arg::with_name("policy")
                        .short("p")
                        .long("policy")
                        .required(false)
                        .takes_value(true)
                        .conflicts_with("input file")
                        .possible_values(&["allow", "deny"])
                        .help("Type of policy"),
                )
                .arg(
                    Arg::with_name("format")
                        .short("f")
                        .long("format")
                        .required(false)
                        .takes_value(true)
                        .possible_values(&["armour", "json", "yaml"])
                        .help("Format of policy"),
                ),
        )
        .get_matches();

    // enable logging
    std::env::set_var("RUST_LOG", "armour_lang=info,actix=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    if let Some(export_matches) = matches.subcommand_matches("export") {
        let policy = match matches.value_of("input file") {
            Some(file) => policies::Policies::from_file(file)?,
            _ => match export_matches.value_of("policy").unwrap_or("deny") {
                "allow" => policies::Policies::allow_all(),
                _ => policies::Policies::deny_all(),
            },
        };
        let s = match export_matches
            .value_of("format")
            .unwrap_or_else(|| "armour")
        {
            "armour" => policy.to_string(),
            "json" => serde_json::to_string_pretty(&policy).unwrap_or_else(|e| e.to_string()),
            "yaml" => serde_yaml::to_string(&policy).unwrap_or_else(|e| e.to_string()),
            _ => unreachable!(),
        };
        print!("{}", s)
    } else {
        // try to load code from an input file
        let file = matches.value_of("input file");
        let mut prog = match file {
            Some(file) => lang::Program::from_file(file)?,
            _ => lang::Program::default(),
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
        prog.print();

        // start eval actor
        let headers = prog.headers.clone();
        let eval = Eval::new(&prog).start();

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
