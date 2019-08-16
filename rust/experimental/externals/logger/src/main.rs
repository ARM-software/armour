use capnp::Error;
use external_server::{Dispatcher, External, Literal};

struct ExternalImpl;

impl Dispatcher for ExternalImpl {
    fn dispatch(&mut self, name: &str, _args: &[Literal]) -> Result<Literal, Error> {
        match name {
            "log" => Ok(Literal::Unit),
            _ => Err(Error::unimplemented(name.to_string())),
        }
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 {
        let socket = args[1].as_str();
        External::start("logger", socket, ExternalImpl)
    } else {
        println!("usage: {} ADDRESS[:PORT]", args[0]);
        Ok(())
    }
}
