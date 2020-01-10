/// Armour policy language
use actix::prelude::*;
use armour_policy::{expressions, interpret::Env, lang};
use clap::{crate_version, App, Arg};
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
    let module: lang::Module;

    // try to load code from an input file
    if let Some(filename) = matches.value_of("input file") {
        module = lang::Module::from_file(filename, None)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    } else {
        module = lang::Module::default()
    }
    let mut prog = module.program;
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

    // start eval actor
    let headers = prog.headers.clone();
    let eval = Eval::new(prog).start();

    // evaluate expressions (REPL)
    let mut rl = Editor::<()>::new();
    if rl.load_history("armour-policy.txt").is_err() {
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
        .save_history("armour-policy.txt")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    {
        log::warn!("{}", e)
    };

    Ok(())
}
