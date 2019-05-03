/// Armour policy language
use arm_policy::lang;
use clap::{crate_version, App, Arg};
use std::fs::File;
use std::io::{prelude::*, stdin, stdout, BufReader};

fn main() -> std::io::Result<()> {
    // Command line interface
    let matches = App::new("Armour")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Armour policy language REPL")
        .arg(
            Arg::with_name("input file")
                .index(1)
                .required(false)
                .help("File to parse"),
        )
        .get_matches();

    // declare program
    let mut prog: lang::Program;

    // try to load code
    if let Some(filename) = matches.value_of("input file") {
        let mut reader = BufReader::new(File::open(filename)?);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        match lang::Program::from_string(&buf) {
            Ok(p) => prog = p,
            Err(err) => return Ok(eprintln!("{}: {}", filename, err)),
        }
    } else {
        prog = lang::Program::new()
    };

    // evaluate expressions (REPL)
    loop {
        print!("> ");
        stdout().flush().unwrap();
        let mut reader = BufReader::new(stdin());
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        match lang::Expr::from_string(&buf, &mut prog.headers) {
            Ok(e) => {
                // println!("{:#?}", e);
                match e.evaluate(&mut prog.code) {
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
