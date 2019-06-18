use capnp::Error;
use external_server::{Dispatcher, External, Literal, Literal::*};

struct ExternalImpl(i64);

impl ExternalImpl {
    fn print_route(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            &[List(_)] => Ok(Literal::Unit),
            _ => Err(Error::failed("print_route".to_string())),
        }
    }
}

impl Dispatcher for ExternalImpl {
    fn dispatch(&mut self, name: &str, args: &[Literal]) -> Result<Literal, Error> {
        match name {
            "print_route" => ExternalImpl::print_route(args),
            _ => Err(Error::unimplemented(name.to_string())),
        }
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 {
        External::start(args[1].as_str(), ExternalImpl(0))
    } else {
        Ok(println!("usage: {} ADDRESS[:PORT]", args[0]))
    }
}
