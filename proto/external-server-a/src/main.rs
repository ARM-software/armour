use capnp::Error;
use external_server::{Dispatcher, External, Literal, Literal::*};

struct ExternalImpl(i64);

impl ExternalImpl {
    fn sin<'a>(args: &[Literal<'a>]) -> Result<Literal<'a>, Error> {
        match args {
            &[FloatLiteral(f)] => Ok(FloatLiteral(f64::sin(f))),
            _ => Err(Error::failed("sin".to_string())),
        }
    }
    fn count(&mut self) -> Result<Literal, Error> {
        self.0 += 1;
        Ok(IntLiteral(self.0))
    }
    fn process<'a>(args: &[Literal<'a>]) -> Result<Literal<'a>, Error> {
        match args {
            &[StringPairs(ref l)] => Ok(IntLiteral(l.len() as i64)),
            _ => Err(Error::failed("process".to_string())),
        }
    }
}

impl Dispatcher for ExternalImpl {
    fn dispatch<'a>(&'a mut self, name: &str, args: &[Literal<'a>]) -> Result<Literal<'a>, Error> {
        match name {
            "sin" => ExternalImpl::sin(args),
            "count" => self.count(),
            "process" => ExternalImpl::process(args),
            _ => Err(Error::unimplemented(name.to_string())),
        }
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 {
        External::start(args[1].clone(), ExternalImpl(0))
    } else {
        Ok(println!("usage: {} ADDRESS[:PORT]", args[0]))
    }
}
