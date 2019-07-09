use capnp::Error;
use external_server::{Dispatcher, External, Literal, Literal::*};

struct ExternalImpl(i64);

impl ExternalImpl {
    fn sin(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            [Float(f)] => Ok(Float(f64::sin(*f))),
            _ => Err(Error::failed("sin".to_string())),
        }
    }
    fn count(&mut self) -> Result<Literal, Error> {
        self.0 += 1;
        Ok(Int(self.0))
    }
    fn rev(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            [List(ref l)] => Ok(List(l.to_vec().into_iter().rev().collect())),
            _ => Err(Error::failed("process".to_string())),
        }
    }
    fn process(args: &[Literal]) -> Result<Literal, Error> {
        match args {
            [List(ref l)] => {
                let v: Result<Vec<Literal>, Error> = l
                    .to_vec()
                    .into_iter()
                    .map(|x| match x {
                        Tuple(v) => v
                            .get(0)
                            .map(|x| x.to_owned())
                            .ok_or_else(|| Error::failed("".to_string())),
                        _ => Err(Error::failed("process".to_string())),
                    })
                    .collect();
                Ok(List(v?))
            }
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
        println!("usage: {} ADDRESS[:PORT]", args[0]);
        Ok(())
    }
}
