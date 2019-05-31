/// Armour policy language
use arm_policy::lang;
use clap::{crate_version, App, Arg};
use std::io::{
    self,
    prelude::{Read, Write},
};
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

    // declare program
    let mut prog: lang::Program;

    // try to load code from an input file
    if let Some(filename) = matches.value_of("input file") {
        prog = lang::Program::from_file(filename)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    } else {
        prog = lang::Program::new()
    };

    // test: serialize then deserialize program (using bincode)
    // let bytes = prog.to_bytes()?;
    // println!("{:?}", bytes);
    // prog = lang::Program::from_bytes(&bytes)?;

    let mut runtime = lang::Runtime::from(&prog);

    if let Some(timeout) = matches.value_of("timeout") {
        let d = Duration::from_secs(timeout.parse().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "timeout (seconds) must be an integer",
            )
        })?);
        runtime.set_timeout(d)
    }

    // evaluate expressions (REPL)
    let mut reader = io::BufReader::new(io::stdin());
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        reader.read_to_string(&mut buf)?;
        match lang::Expr::from_string(&buf, &prog.headers) {
            Ok(e) => {
                // println!("{:#?}", e);
                match runtime.evaluate(e) {
                    Ok(r) => println!(": {}", r),
                    Err(err) => eprintln!("{}", err),
                }
            }
            Err(err) => {
                if buf == "" {
                    break;
                } else {
                    eprintln!("{}", err)
                }
            }
        }
    }

    // done
    Ok(())
}
