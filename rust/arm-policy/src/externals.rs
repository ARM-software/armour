use super::literals::Literal;
use crate::external_capnp::external;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{future, Future};
use futures_timer::FutureExt;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};

use actix::prelude::*;

#[derive(Clone)]
struct CallRequest {
    external: String,
    method: String,
    args: Vec<Literal>,
}

impl CallRequest {
    pub fn new(external: &str, method: &str, args: Vec<Literal>) -> CallRequest {
        CallRequest {
            external: external.to_string(),
            method: method.to_string(),
            args,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    ClientAlreadyExists,
    ClientMissing,
    RequestNotValid,
    Socket,
    IO(std::io::Error),
    Capnp(capnp::Error),
    #[cfg(not(target_env = "musl"))]
    NativeTLS(native_tls::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<capnp::Error> for Error {
    fn from(err: capnp::Error) -> Error {
        Error::Capnp(err)
    }
}

pub trait PathOrToSocketAddrs<T: std::net::ToSocketAddrs, P: AsRef<std::path::Path>> {
    fn get_to_socket_addrs(&self) -> Option<T>;
    fn get_path(&self) -> Option<P>;
}

impl<'a> PathOrToSocketAddrs<&'a str, &'a str> for &'a str {
    fn get_to_socket_addrs(&self) -> Option<&'a str> {
        if self.to_socket_addrs().is_ok() {
            Some(self)
        } else {
            None
        }
    }
    fn get_path(&self) -> Option<&'a str> {
        Some(self)
    }
}

pub struct Externals {
    externals: HashMap<String, String>,
    timeout: Duration,
}

const TIMEOUT: Duration = Duration::from_secs(3);

impl Default for Externals {
    fn default() -> Self {
        Externals {
            externals: HashMap::new(),
            timeout: TIMEOUT,
        }
    }
}

impl Externals {
    fn get_tls_stream<T: std::net::ToSocketAddrs>(
        socket: T,
    ) -> Box<dyn Future<Item = tokio_tls::TlsStream<tokio::net::TcpStream>, Error = Error>> {
        match socket.to_socket_addrs() {
            Ok(mut iter) => {
                if let Some(addr) = iter.next() {
                    let mut builder = native_tls::TlsConnector::builder();
                    #[cfg(debug_assertions)]
                    builder.danger_accept_invalid_certs(true);
                    let tls_connector = tokio_tls::TlsConnector::from(
                        builder.build().expect("failed to create TLS connector"),
                    );
                    let tls = tokio::net::TcpStream::connect(&addr).and_then(move |sock| {
                        sock.set_nodelay(true).expect("failed to set nodelay");
                        tls_connector
                            .connect(&addr.to_string(), sock)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    });
                    Box::new(tls.from_err())
                } else {
                    Box::new(future::err(Error::Socket))
                }
            }
            Err(err) => Box::new(future::err(Error::from(err))),
        }
    }
    fn get_tcp_stream<T: std::net::ToSocketAddrs>(
        socket: T,
    ) -> Box<dyn Future<Item = tokio::net::TcpStream, Error = Error>> {
        match socket.to_socket_addrs() {
            Ok(mut iter) => {
                if let Some(addr) = iter.next() {
                    Box::new(
                        tokio::net::TcpStream::connect(&addr)
                            .and_then(|sock| match sock.set_nodelay(true) {
                                Ok(_) => future::ok(sock),
                                Err(err) => future::err(err),
                            })
                            .from_err(),
                    )
                } else {
                    Box::new(future::err(Error::Socket))
                }
            }
            Err(err) => Box::new(future::err(Error::from(err))),
        }
    }
    fn get_uds_stream<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Box<dyn Future<Item = tokio_uds::UnixStream, Error = Error> + Send> {
        Box::new(tokio_uds::UnixStream::connect(path).from_err())
    }
    pub fn get_capnp_client<T: 'static>(
        stream: T,
    ) -> (
        external::Client,
        capnp_rpc::RpcSystem<capnp_rpc::rpc_twoparty_capnp::Side>,
    )
    where
        T: AsyncRead + AsyncWrite,
    {
        let (reader, writer) = stream.split();
        let network = Box::new(twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Client,
            Default::default(),
        ));
        let mut rpc_system = RpcSystem::new(network, None);
        let client: external::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
        (client, rpc_system)
    }
    fn build_request(
        req: &CallRequest,
        client: &external::Client,
    ) -> Result<
        capnp::capability::Request<external::call_params::Owned, external::call_results::Owned>,
        Error,
    > {
        // prepare the RPC
        let mut call_req = client.call_request();
        let mut call = call_req.get();
        // set the name
        call.set_name(req.method.as_str());
        // set the args
        let mut args = call.init_args(req.args.len() as u32);
        for (i, lit) in req.args.iter().enumerate() {
            let mut arg = args.reborrow().get(i as u32);
            match lit {
                Literal::BoolLiteral(b) => arg.set_bool(*b),
                Literal::IntLiteral(i) => arg.set_int64(*i),
                Literal::FloatLiteral(f) => arg.set_float64(*f),
                Literal::StringLiteral(s) => arg.set_text(s),
                Literal::DataLiteral(d) => arg.set_data(d),
                Literal::Unit => arg.set_unit(()),
                Literal::List(lits) => {
                    let mut pairs = arg.init_pairs(lits.len() as u32);
                    for (j, l) in lits.iter().enumerate() {
                        match l {
                            Literal::Tuple(ts) => match ts.as_slice() {
                                &[Literal::StringLiteral(ref key), Literal::DataLiteral(ref value)] =>
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
        Ok(call_req)
    }
    fn call_request(
        req: CallRequest,
        timeout: Duration,
        client: external::Client,
    ) -> Box<dyn Future<Item = Literal, Error = Error>> {
        use external::value::Which;
        Box::new(
            // prepare the RPC
            Externals::build_request(&req, &client)
                .unwrap()
                // make the RPC
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
                            Which::Data(d) => Literal::DataLiteral(pry!(d).to_vec()),
                            Which::Unit(_) => Literal::Unit,
                            Which::Lines(ps) => {
                                let mut v = Vec::new();
                                for p in pry!(ps) {
                                    v.push(Literal::StringLiteral(pry!(p).to_string()))
                                }
                                Literal::List(v)
                            }
                            Which::Pairs(ps) => {
                                let mut v = Vec::new();
                                for p in pry!(ps) {
                                    v.push(Literal::Tuple(vec![
                                        Literal::StringLiteral(pry!(p.get_key()).to_string()),
                                        Literal::DataLiteral(pry!(p.get_value()).to_vec()),
                                    ]))
                                }
                                Literal::List(v)
                            }
                        },
                    )
                })
                .timeout(timeout)
                .or_else(|err| Promise::err(Error::from(err))),
        )
    }
    pub fn register_external(&mut self, name: &str, addr: &str) -> bool {
        self.externals
            .insert(name.to_string(), addr.to_string())
            .is_some()
    }
    pub fn request(
        &self,
        external: &str,
        method: &str,
        args: Vec<Literal>,
    ) -> Result<Literal, Error> {
        if let Some(socket) = self.externals.get(external) {
            let mut sys = System::new("arm-policy");
            println!("going to make call to: {}", socket);
            let res = if let Some(p) = socket.as_str().get_to_socket_addrs() {
                if cfg!(target_env = "musl") {
                    // for musl builds we are not able to use TLS (too much hassle with OpenSSL)
                    sys.block_on(Externals::get_tcp_stream(p).and_then(|stream| {
                        let (client, rpc_system) = Externals::get_capnp_client(stream);
                        let disconnector = rpc_system.get_disconnector();
                        actix::spawn(rpc_system.timeout(self.timeout).map_err(|_| ()));
                        Externals::call_request(
                            CallRequest::new(external, method, args),
                            self.timeout,
                            client,
                        )
                        .then(|res| {
                            disconnector.then(|_| {
                                actix::System::current().stop();
                                res
                            })
                        })
                    }))
                } else {
                    // for non-musl builds we use TLS
                    sys.block_on(Externals::get_tls_stream(p).and_then(|stream| {
                        let (client, rpc_system) = Externals::get_capnp_client(stream);
                        let disconnector = rpc_system.get_disconnector();
                        actix::spawn(rpc_system.timeout(self.timeout).map_err(|_| ()));
                        Externals::call_request(
                            CallRequest::new(external, method, args),
                            self.timeout,
                            client,
                        )
                        .then(|res| {
                            disconnector.then(|_| {
                                actix::System::current().stop();
                                res
                            })
                        })
                    }))
                }
            } else if let Some(p) = socket.as_str().get_path() {
                sys.block_on(Externals::get_uds_stream(p).and_then(|stream| {
                    let (client, rpc_system) = Externals::get_capnp_client(stream);
                    let disconnector = rpc_system.get_disconnector();
                    actix::spawn(rpc_system.timeout(self.timeout).map_err(|_| ()));
                    Externals::call_request(
                        CallRequest::new(external, method, args),
                        self.timeout,
                        client,
                    )
                    // .and_then(|res| disconnector.from_err().and_then(|_| future::ok(res)))
                    .then(|res| {
                        disconnector.then(|_| {
                            actix::System::current().stop();
                            res
                        })
                    })
                }))
            } else {
                Err(Error::IO(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("failed to parse TCP socket or path: {}", socket),
                )))
            };
            sys.run();
            res
        } else {
            Err(Error::ClientMissing)
        }
    }
}
