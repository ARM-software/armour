use super::literals::Literal;
use crate::external_capnp::external;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::Future;
use std::collections::HashMap;
use tokio::io::AsyncRead;

struct CallRequest<'a> {
    external: &'a str,
    method: &'a str,
    args: Vec<Literal>,
}

impl<'a> CallRequest<'a> {
    pub fn new(external: &'a str, method: &'a str, args: Vec<Literal>) -> CallRequest<'a> {
        CallRequest {
            external,
            method,
            args,
        }
    }
}

pub struct Externals {
    clients: HashMap<String, external::Client>,
    runtime: tokio::runtime::current_thread::Runtime,
}

#[derive(Debug)]
pub enum Error {
    ClientAlreadyExists,
    ClientMissing,
    RequestNotValid,
    IO,
    Socket,
    Capnp(capnp::Error),
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Error {
        Error::IO
    }
}

impl From<capnp::Error> for Error {
    fn from(err: capnp::Error) -> Error {
        Error::Capnp(err)
    }
}

impl Externals {
    pub fn new() -> Externals {
        Externals {
            clients: HashMap::new(),
            runtime: tokio::runtime::current_thread::Runtime::new()
                .expect("failed to initiate tokio runtime"),
        }
    }
    pub fn add_client<T: std::net::ToSocketAddrs>(
        &mut self,
        name: &str,
        socket: T,
    ) -> Result<(), Error> {
        if self.clients.contains_key(name) {
            Err(Error::ClientAlreadyExists)
        } else if let Some(addr) = socket.to_socket_addrs()?.next() {
            let stream = self
                .runtime
                .block_on(tokio::net::TcpStream::connect(&addr))?;
            stream.set_nodelay(true)?;
            let (reader, writer) = stream.split();
            let network = Box::new(twoparty::VatNetwork::new(
                reader,
                writer,
                rpc_twoparty_capnp::Side::Client,
                Default::default(),
            ));
            let mut rpc_system = RpcSystem::new(network, None);
            let client: external::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
            // let disconnector = rpc_system.get_disconnector();
            // self.runtime.block_on(disconnector)?;
            self.clients.insert(name.to_string(), client);
            self.runtime.spawn(rpc_system.map_err(|_e| ()));
            Ok(())
        } else {
            Err(Error::Socket)
        }
    }
    fn call_request(&mut self, req: &CallRequest) -> Result<Literal, Error> {
        use external::value::Which;
        if let Some(client) = self.clients.get(req.external) {
            // prepare the RPC
            let mut call_req = client.call_request();
            let mut call = call_req.get();
            // set the name
            call.set_name(req.method);
            // set the args
            let mut args = call.init_args(req.args.len() as u32);
            for (i, lit) in req.args.iter().enumerate() {
                let mut arg = args.reborrow().get(i as u32);
                match lit {
                    Literal::BoolLiteral(b) => arg.set_bool(*b),
                    Literal::IntLiteral(i) => arg.set_int64(*i),
                    Literal::FloatLiteral(f) => arg.set_float64(*f),
                    Literal::StringLiteral(s) => arg.set_text(s),
                    Literal::DataLiteral(d) => arg.set_data(d.as_bytes()),
                    Literal::Unit => arg.set_unit(()),
                    Literal::List(lits) => {
                        let mut pairs = arg.init_pairs(lits.len() as u32);
                        for (j, l) in lits.iter().enumerate() {
                            match l {
                                Literal::Tuple(ts) => match ts.as_slice() {
                                    &[Literal::StringLiteral(ref key), Literal::StringLiteral(ref value)] =>
                                    {
                                        let mut pair = pairs.reborrow().get(j as u32);
                                        pair.set_key(key);
                                        pair.set_value(value);
                                    }
                                    _ => return Err(Error::RequestNotValid),
                                },
                                _ => return Err(Error::RequestNotValid),
                            }
                        }
                    }
                    _ => return Err(Error::RequestNotValid),
                }
            }
            // make the RPC
            self.runtime.block_on(
                call_req
                    .send()
                    .promise
                    .and_then(|response| {
                        // return the result
                        Promise::ok(
                            match pry!(pry!(pry!(response.get()).get_result()).which()) {
                                Which::Bool(b) => Literal::BoolLiteral(b),
                                Which::Int64(i) => Literal::IntLiteral(i),
                                Which::Float64(f) => Literal::FloatLiteral(f),
                                Which::Text(t) => Literal::StringLiteral(pry!(t).to_string()),
                                Which::Data(d) => Literal::DataLiteral(
                                    String::from_utf8_lossy(pry!(d)).to_string(),
                                ),
                                Which::Unit(_) => Literal::Unit,
                                Which::Pairs(ps) => {
                                    let mut v = Vec::new();
                                    for p in pry!(ps) {
                                        v.push(Literal::Tuple(vec![
                                            Literal::StringLiteral(pry!(p.get_key()).to_string()),
                                            Literal::StringLiteral(pry!(p.get_value()).to_string()),
                                        ]))
                                    }
                                    Literal::List(v)
                                }
                            },
                        )
                    })
                    .or_else(|err| Promise::err(Error::from(err))),
            )
        } else {
            Err(Error::ClientMissing)
        }
    }
    pub fn request(
        &mut self,
        external: &str,
        method: &str,
        args: Vec<Literal>,
    ) -> Result<Literal, Error> {
        self.call_request(&CallRequest::new(external, method, args))
    }
    pub fn print_request(&mut self, external: &str, method: &str, args: Vec<Literal>) {
        match self.call_request(&CallRequest::new(external, method, args)) {
            Ok(result) => println!("{}", result),
            Err(err) => eprintln!("{:?}", err),
        }
    }
}
