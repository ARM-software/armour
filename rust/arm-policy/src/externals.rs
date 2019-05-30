use super::literals::Literal;
use crate::external_capnp::external;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::Future;
use futures_timer::FutureExt;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};

const TIMEOUT: Duration = Duration::from_secs(3);

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
    timeout: Duration,
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

impl Externals {
    pub fn new() -> Externals {
        Externals {
            clients: HashMap::new(),
            runtime: tokio::runtime::current_thread::Runtime::new()
                .expect("failed to initiate tokio runtime"),
            timeout: TIMEOUT,
        }
    }
    pub fn set_timeout(&mut self, t: Duration) {
        self.timeout = t
    }
    #[cfg(not(target_env = "musl"))]
    fn get_tls_stream<T: std::net::ToSocketAddrs>(
        &mut self,
        socket: T,
    ) -> Result<tokio_tls::TlsStream<tokio::net::TcpStream>, Error> {
        if let Some(addr) = socket.to_socket_addrs()?.next() {
            let mut builder = native_tls::TlsConnector::builder();
            #[cfg(debug_assertions)]
            builder.danger_accept_invalid_certs(true);
            let tls_connector = tokio_tls::TlsConnector::from(
                builder.build().expect("failed to create TLS connector"),
            );
            let tls = tokio::net::TcpStream::connect(&addr).and_then(|sock| {
                sock.set_nodelay(true).expect("failed to set nodelay");
                tls_connector
                    .connect(&addr.to_string(), sock)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });
            self.runtime.block_on(tls).map_err(Error::from)
        } else {
            Err(Error::Socket)
        }
    }
    #[cfg(target_env = "musl")]
    fn get_tcp_stream<T: std::net::ToSocketAddrs>(
        &mut self,
        socket: T,
    ) -> Result<tokio::net::TcpStream, Error> {
        if let Some(addr) = socket.to_socket_addrs()?.next() {
            let stream = self
                .runtime
                .block_on(tokio::net::TcpStream::connect(&addr))?;
            stream.set_nodelay(true)?;
            Ok(stream)
        } else {
            Err(Error::Socket)
        }
    }
    fn get_uds_stream<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<tokio_uds::UnixStream, Error> {
        let stream = self
            .runtime
            .block_on(tokio_uds::UnixStream::connect(path))?;
        Ok(stream)
    }
    pub fn add_client_stream<'a, 'b, T: 'static + 'a>(
        &'a mut self,
        name: &'b str,
        stream: T,
    ) -> Result<(), Error>
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
        // let disconnector = rpc_system.get_disconnector();
        // self.runtime.block_on(disconnector)?;
        self.clients.insert(name.to_string(), client);
        self.runtime.spawn(rpc_system.map_err(|_| ()));
        Ok(())
    }
    pub fn add_client<S, P, T>(&mut self, name: &str, socket: T) -> Result<(), Error>
    where
        S: std::net::ToSocketAddrs,
        P: AsRef<std::path::Path>,
        T: PathOrToSocketAddrs<S, P> + std::fmt::Display,
    {
        if self.clients.contains_key(name) {
            Err(Error::ClientAlreadyExists)
        } else if let Some(p) = socket.get_to_socket_addrs() {
            #[cfg(target_env = "musl")]
            let stream = self.get_tcp_stream(p)?;
            #[cfg(not(target_env = "musl"))]
            let stream = self.get_tls_stream(p)?;
            self.add_client_stream(name, stream)
        } else if let Some(p) = socket.get_path() {
            let stream = self.get_uds_stream(p)?;
            self.add_client_stream(name, stream)
        } else {
            Err(Error::IO(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to parse TCP socket or path: {}", socket),
            )))
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
                Literal::DataLiteral(d) => arg.set_data(d),
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
        Ok(call_req)
    }
    fn call_request(&mut self, req: &CallRequest) -> Result<Literal, Error> {
        use external::value::Which;
        if let Some(client) = self.clients.get(req.external) {
            // prepare the RPC
            let call_req = Externals::build_request(req, client)?;
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
                                            Literal::StringLiteral(pry!(p.get_value()).to_string()),
                                        ]))
                                    }
                                    Literal::List(v)
                                }
                            },
                        )
                    })
                    .timeout(self.timeout)
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
