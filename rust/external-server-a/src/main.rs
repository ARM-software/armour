use capnp::Error;
use external_server::{Dispatcher, External, Literal, Literal::*, MapEntry};

struct ExternalImpl(i64);

impl ExternalImpl {
    fn sin(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            &[FloatLiteral(f)] => Ok(FloatLiteral(f64::sin(f))),
            _ => Err(Error::failed("sin".to_string())),
        }
    }
    fn count(&mut self) -> Result<Literal, Error> {
        self.0 += 1;
        Ok(IntLiteral(self.0))
    }
    fn rev(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            &[StringMap(ref l)] => Ok(StringMap(l.to_vec().into_iter().rev().collect())),
            _ => Err(Error::failed("process".to_string())),
        }
    }
    fn process(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            &[StringMap(ref l)] => Ok(StringMap(
                l.to_vec()
                    .into_iter()
                    .map(|x| (x.0, MapEntry::Unit))
                    .collect(),
            )),
            _ => Err(Error::failed("process".to_string())),
        }
    }
}

impl Dispatcher for ExternalImpl {
    fn dispatch(&mut self, name: &str, args: &[Literal]) -> Result<Literal, Error> {
        match name {
            "sin" => ExternalImpl::sin(args),
            "count" => self.count(),
            "rev" => ExternalImpl::rev(args),
            "process" => ExternalImpl::process(args),
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
