/// Armour policy language
use actix::prelude::*;
use armour_lang::{
    expressions, 
    interpret::{Env}, 
    lang, 
    literals::{self, TFlatLiteral}, 
    policies, 
    types::{self, TFlatTyp}
};
use clap::{crate_version, App, Arg, SubCommand, ArgMatches};
use rustyline::{error::ReadlineError, Editor};
use std::io;
use std::time::Duration;

struct Eval<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static> {
    env: Env<FlatTyp, FlatLiteral>,
}

impl<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static> Eval<FlatTyp, FlatLiteral> {
    fn new(prog: &lang::Program<FlatTyp, FlatLiteral>) -> Self {
        Eval {
            env: Env::new(prog),
        }
    }
}

impl<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static> Actor for Eval<FlatTyp, FlatLiteral> {
    type Context = Context<Self>;
}

//#[derive(Message)]
//#[rtype(result = "Result<expressions::Expr<FlatTyp, FlatLiteral>, expressions::Error<FlatTyp, FlatLiteral>>")]
struct Evaluate<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static>(expressions::Expr<FlatTyp, FlatLiteral>);

impl<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static> Message for Evaluate<FlatTyp, FlatLiteral>{
    type Result = Result<expressions::Expr<FlatTyp, FlatLiteral>, expressions::Error>;
}

impl<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static> Handler<Evaluate<FlatTyp, FlatLiteral>> for Eval<FlatTyp, FlatLiteral> {
    type Result = ResponseFuture<Result<expressions::Expr<FlatTyp, FlatLiteral>, expressions::Error>>;
    fn handle(&mut self, msg: Evaluate<FlatTyp, FlatLiteral>, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(msg.0.evaluate(self.env.clone()))
    }
}

struct Stop;

impl Message for Stop {
    type Result = ();
}

impl<FlatTyp: TFlatTyp+'static, FlatLiteral: TFlatLiteral<FlatTyp>+'static> Handler<Stop> for Eval<FlatTyp, FlatLiteral> {
    type Result = ();
    fn handle(&mut self, _msg: Stop, _ctx: &mut Context<Self>) {
        System::current().stop()
    }
}


fn load_from_file<FlatTyp: TFlatTyp, FlatLiteral: TFlatLiteral<FlatTyp>>(matches :ArgMatches) -> std::io::Result<lang::Program<FlatTyp, FlatLiteral>> {
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
    Ok(prog)
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
            SubCommand::with_name("dataplane")
                .about("")
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
        )
        .subcommand(
            SubCommand::with_name("controlplane")
                .about("")
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
                )

        )
        .get_matches();

    // enable logging
    std::env::set_var("RUST_LOG", "armour_lang=info,actix=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();


    if let Some(dataplane_matches) = matches.subcommand_matches("dataplane") {
        if let Some(export_matches) = dataplane_matches.subcommand_matches("export") {
            let policy: policies::DPPolicies = match matches.value_of("input file") {
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
            let prog = load_from_file::<types::FlatTyp, literals::DPFlatLiteral>(matches)?; 

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
    } else if let Some(control_matches) = matches.subcommand_matches("controlplane") {
        //let prog = load_from_file::<types_cp::CPFlatTyp, literals::CPFlatLiteral>(matches)?; 

        //TODO compile/specialization
        unimplemented!()
    }
    Ok(())
}
