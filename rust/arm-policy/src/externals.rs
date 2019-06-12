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

#[derive(Clone)]
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
    pub fn set_timeout(&mut self, t: Duration) {
        self.timeout = t
    }
    pub fn timeout(&self) -> Duration {
        self.timeout.clone()
    }
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
    fn build_value(mut v: external::value::Builder<'_>, lit: &Literal) -> Result<(), Error> {
        match lit {
            Literal::BoolLiteral(b) => v.set_bool(*b),
            Literal::IntLiteral(i) => v.set_int64(*i),
            Literal::FloatLiteral(f) => v.set_float64(*f),
            Literal::StringLiteral(s) => v.set_text(s),
            Literal::DataLiteral(d) => v.set_data(d),
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
            _ => return Err(Error::RequestNotValid),
        }
        Ok(())
    }
    fn read_value(v: external::value::Reader<'_>) -> Result<Literal, capnp::Error> {
        use external::value::Which;
        match v.which() {
            Ok(Which::Bool(b)) => Ok(Literal::BoolLiteral(b)),
            Ok(Which::Int64(i)) => Ok(Literal::IntLiteral(i)),
            Ok(Which::Float64(f)) => Ok(Literal::FloatLiteral(f)),
            Ok(Which::Text(t)) => Ok(Literal::StringLiteral(t?.to_string())),
            Ok(Which::Data(d)) => Ok(Literal::DataLiteral(d?.to_vec())),
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
            Externals::build_value(args.reborrow().get(i as u32), lit)?
        }
        Ok(call_req)
    }
    fn call_request(
        req: CallRequest,
        timeout: Duration,
        client: external::Client,
    ) -> Box<dyn Future<Item = Literal, Error = Error>> {
        match Externals::build_request(&req, &client) {
            Ok(req) => {
                Box::new(
                    // prepare the RPC
                    req
                        // make the RPC
                        .send()
                        .promise
                        .and_then(|response| {
                            // return the result
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
    pub fn register_external(&mut self, name: &str, addr: &str) -> bool {
        self.externals
            .insert(name.to_string(), addr.to_string())
            .is_some()
    }
    pub fn get_socket(&self, external: &str) -> Option<String> {
        self.externals.get(external).map(|x| x.clone())
    }
    pub fn request(
        external: String,
        method: String,
        args: Vec<Literal>,
        socket: String,
        timeout: Duration,
    ) -> Box<dyn Future<Item = Literal, Error = Error>> {
        println!("making call to: {}", socket);
        if let Some(p) = socket.as_str().get_to_socket_addrs() {
            if cfg!(target_env = "musl") {
                // for musl builds we are not able to use TLS (too much hassle with OpenSSL)
                Box::new(Externals::get_tcp_stream(p).and_then(move |stream| {
                    let (client, rpc_system) = Externals::get_capnp_client(stream);
                    actix::spawn(rpc_system.map_err(|_| ()));
                    Externals::call_request(
                        CallRequest::new(&external, &method, args),
                        timeout,
                        client,
                    )
                }))
            } else {
                // for non-musl builds we use TLS
                Box::new(Externals::get_tls_stream(p).and_then(move |stream| {
                    let (client, rpc_system) = Externals::get_capnp_client(stream);
                    actix::spawn(rpc_system.map_err(|_| ()));
                    Externals::call_request(
                        CallRequest::new(&external, &method, args),
                        timeout,
                        client,
                    )
                }))
            }
        } else if let Some(p) = socket.as_str().get_path() {
            Box::new(Externals::get_uds_stream(p).and_then(move |stream| {
                let (client, rpc_system) = Externals::get_capnp_client(stream);
                // let disconnector = rpc_system.get_disconnector();
                // println!("spawning RPC");
                actix::spawn(rpc_system.map_err(|_| ()));
                Externals::call_request(CallRequest::new(&external, &method, args), timeout, client)
                // .then(|res| {
                //     println!("got result and going to disconnect");
                //     disconnector.then(|_| res)
                // })
            }))
        } else {
            Box::new(future::err(Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to parse TCP socket or path: {}", socket),
            ))))
        }
    }
}
