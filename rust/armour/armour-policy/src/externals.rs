use super::literals::{Literal, ToLiteral};
use crate::external_capnp::external;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{future, Future};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::FutureExt;

/// Calls to Armour external functions
///
/// Calls take the form `external::method(args)`
#[derive(Clone)]
struct Call {
    external: String,
    method: String,
    args: Vec<Literal>,
}

impl Call {
    /// Call constructor
    pub fn new(external: &str, method: &str, args: Vec<Literal>) -> Call {
        Call {
            external: external.to_string(),
            method: method.to_string(),
            args,
        }
    }
}

/// Errors that can occur when trying to connect with externals
#[derive(Debug)]
pub enum Error {
    Failed(String),
    IO(std::io::Error),
    Capnp(capnp::Error),
}

impl From<&str> for Error {
    fn from(err: &str) -> Error {
        Error::Failed(err.to_string())
    }
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

impl From<tokio_timer::timeout::Error<capnp::Error>> for Error {
    fn from(err: tokio_timer::timeout::Error<capnp::Error>) -> Error {
        err.into_inner()
            .map(Error::Capnp)
            .unwrap_or_else(|| Error::Failed("timeout".to_string()))
    }
}

/// Trait for structures that can be converted either to a TCP socket address or to a Unix socket path
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

#[derive(Serialize, Deserialize, Clone)]
pub struct Externals {
    /// map from external names to TCP/Unix socket names
    externals: HashMap<String, String>,
    /// time limit and external calls
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
    pub fn set_timeout(&mut self, t: Duration) {
        self.timeout = t
    }
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
    pub fn add_external(&mut self, name: &str, addr: &str) -> bool {
        self.externals
            .insert(name.to_string(), addr.to_string())
            .is_some()
    }
    fn get_tls_stream<T: std::net::ToSocketAddrs>(
        socket: T,
    ) -> Box<dyn Future<Item = tokio_tls::TlsStream<tokio::net::TcpStream>, Error = Error>> {
        match socket.to_socket_addrs() {
            Ok(mut iter) => {
                if let Some(addr) = iter.next() {
                    #[allow(unused_mut)]
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
                    Box::new(future::err(Error::from("socket address")))
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
    /// Build a Cap'n Proto literal from an Armour literal
    fn build_value(mut v: external::value::Builder<'_>, lit: &Literal) -> Result<(), Error> {
        match lit {
            Literal::Bool(b) => v.set_bool(*b),
            Literal::Int(i) => v.set_int64(*i),
            Literal::Float(f) => v.set_float64(*f),
            Literal::Str(s) => v.set_text(s),
            Literal::Data(d) => v.set_data(d),
            Literal::Unit => v.set_unit(()),
            Literal::Tuple(ts) => {
                let mut tuple = v.init_tuple(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(tuple.reborrow().get(i as u32), t)?
                }
            }
            Literal::List(ts) => {
                let mut list = v.init_list(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(list.reborrow().get(i as u32), t)?
                }
            }
            Literal::HttpRequest(req) => Externals::build_value(v, &req.to_literal())?,
            Literal::ID(id) => Externals::build_value(v, &id.to_literal())?,
            Literal::IpAddr(ip) => Externals::build_value(v, &ip.to_literal())?,
            Literal::Policy(p) => v.set_text(p.to_string().as_str()),
        }
        Ok(())
    }
    /// Read a Cap'n Proto literal and return an Armour literal
    fn read_value(v: external::value::Reader<'_>) -> Result<Literal, capnp::Error> {
        use external::value::Which;
        match v.which() {
            Ok(Which::Bool(b)) => Ok(Literal::Bool(b)),
            Ok(Which::Int64(i)) => Ok(Literal::Int(i)),
            Ok(Which::Float64(f)) => Ok(Literal::Float(f)),
            Ok(Which::Text(t)) => Ok(Literal::Str(t?.to_string())),
            Ok(Which::Data(d)) => Ok(Literal::Data(d?.to_vec())),
            Ok(Which::Unit(_)) => Ok(Literal::Unit),
            Ok(Which::Tuple(ts)) => {
                let mut tuple = Vec::new();
                for t in ts? {
                    tuple.push(Externals::read_value(t)?)
                }
                Ok(Literal::Tuple(tuple))
            }
            Ok(Which::List(ts)) => {
                let mut list = Vec::new();
                for t in ts? {
                    list.push(Externals::read_value(t)?)
                }
                Ok(Literal::List(list))
            }
            Err(e) => Err(capnp::Error::from(e)),
        }
    }
    fn build_request(
        call: &Call,
        client: &external::Client,
    ) -> Result<
        capnp::capability::Request<external::call_params::Owned, external::call_results::Owned>,
        Error,
    > {
        // prepare the RPC
        let mut call_req = client.call_request();
        let mut call_builder = call_req.get();
        // set the name
        call_builder.set_name(call.method.as_str());
        // set the args
        let mut args = call_builder.init_args(call.args.len() as u32);
        for (i, lit) in call.args.iter().enumerate() {
            Externals::build_value(args.reborrow().get(i as u32), lit)?
        }
        Ok(call_req)
    }
    fn call_request(
        call: Call,
        timeout: Duration,
        client: external::Client,
    ) -> Box<dyn Future<Item = Literal, Error = Error>> {
        match Externals::build_request(&call, &client) {
            Ok(req) => {
                Box::new(
                    // make the RPC and turn the result into a promise
                    req.send()
                        .promise
                        .and_then(|response| {
                            match Externals::read_value(pry!(pry!(response.get()).get_result())) {
                                Ok(lit) => Promise::ok(lit),
                                Err(err) => Promise::err(err),
                            }
                        })
                        .timeout(timeout)
                        .or_else(|err| Promise::err(Error::from(err))),
                )
            }
            Err(err) => Box::new(future::err(err)),
        }
    }
    pub fn get_socket(&self, external: &str) -> Option<String> {
        self.externals.get(external).cloned()
    }
    pub fn request(
        external: String,
        method: String,
        args: Vec<Literal>,
        socket: String,
        timeout: Duration,
    ) -> Box<dyn Future<Item = Literal, Error = Error>> {
        log::debug!("making call to: {}", socket);
        if let Some(p) = socket.as_str().get_to_socket_addrs() {
            Box::new(Externals::get_tls_stream(p).and_then(move |stream| {
                let (client, rpc_system) = Externals::get_capnp_client(stream);
                actix::spawn(rpc_system.map_err(|_| ()));
                Externals::call_request(Call::new(&external, &method, args), timeout, client)
            }))
        } else if let Some(p) = socket.as_str().get_path() {
            Box::new(Externals::get_uds_stream(p).and_then(move |stream| {
                let (client, rpc_system) = Externals::get_capnp_client(stream);
                actix::spawn(rpc_system.map_err(|_| ()));
                Externals::call_request(Call::new(&external, &method, args), timeout, client)
            }))
        } else {
            Box::new(future::err(Error::from(
                format!("could not parse TCP socket or path: {}", socket).as_str(),
            )))
        }
    }
}
