use super::expressions::{self, Expr};
use super::literals::Literal;
use crate::external_capnp::external;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{
    future,
    stream::{self, Stream},
    Future,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::FutureExt;

pub struct Call<'a> {
    external: &'a str,
    method: &'a str,
    args: Vec<Literal>,
    timeout: Duration,
}

impl<'a> Call<'a> {
    pub fn new(external: &'a str, method: &'a str, args: Vec<Literal>, timeout: Duration) -> Self {
        Call {
            external,
            method,
            args,
            timeout,
        }
    }
}

pub type Disconnector = Box<dyn Future<Item = (), Error = ()>>;

#[derive(Default)]
pub struct ExternalClients {
    clients: HashMap<String, external::Client>,
}

impl ExternalClients {
    pub fn connect(
        e: Externals,
    ) -> Box<dyn Future<Item = (Self, Vec<Disconnector>), Error = std::io::Error>> {
        Box::new(
            stream::futures_unordered(e.externals.into_iter().map(|(name, socket)| {
                ExternalClients::client(socket.to_string()).and_then(|res| future::ok((name, res)))
            }))
            .collect()
            .and_then(|res| {
                let mut clients = HashMap::new();
                let mut disconnectors = Vec::new();
                for (name, (client, disconnector)) in res.into_iter() {
                    clients.insert(name, client);
                    disconnectors.push(disconnector);
                }
                future::ok((ExternalClients { clients }, disconnectors))
            }),
        )
    }
    pub fn client(
        socket: String,
    ) -> Box<dyn Future<Item = (external::Client, Disconnector), Error = std::io::Error>> {
        if let Ok(mut p) = socket.to_socket_addrs() {
            Box::new(
                tokio::net::TcpStream::connect(&p.next().unwrap())
                    .and_then(|stream| future::ok(ExternalClients::get(stream))),
            )
        } else {
            Box::new(
                tokio_uds::UnixStream::connect(socket.clone())
                    .and_then(|stream| future::ok(ExternalClients::get(stream)))
                    .map_err(move |e| std::io::Error::new(e.kind(), format!("{}: {}", socket, e))),
            )
        }
    }
    fn get<T: 'static>(stream: T) -> (external::Client, Disconnector)
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
        let client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
        // TODO: provide diconnect facility
        let disconnector = rpc_system.get_disconnector();
        let f = Box::new(disconnector.then(|_| future::ok(())));
        actix::spawn(rpc_system.map_err(|_| ()));
        (client, f)
    }
    /// Build a Cap'n Proto literal from an Armour literal
    fn build_value(mut v: external::value::Builder<'_>, lit: &Literal) {
        match lit {
            Literal::Bool(b) => v.set_bool(*b),
            Literal::Connection(conn) => ExternalClients::build_value(v, &Literal::from(conn)),
            Literal::Data(d) => v.set_data(d),
            Literal::Float(f) => v.set_float64(*f),
            Literal::HttpRequest(req) => ExternalClients::build_value(v, &Literal::from(req)),
            Literal::HttpResponse(res) => ExternalClients::build_value(v, &Literal::from(res)),
            Literal::ID(id) => ExternalClients::build_value(v, &Literal::from(id)),
            Literal::Int(i) => v.set_int64(*i),
            Literal::IpAddr(ip) => ExternalClients::build_value(v, &Literal::from(ip)),
            Literal::Payload(pld) => ExternalClients::build_value(v, &Literal::from(pld)),
            Literal::Regex(r) => v.set_text(&r.to_string()),
            Literal::Str(s) => v.set_text(s),
            Literal::Unit => v.set_unit(()),
            Literal::Tuple(ts) => {
                let mut tuple = v.init_tuple(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    ExternalClients::build_value(tuple.reborrow().get(i as u32), t)
                }
            }
            Literal::List(ts) => {
                let mut list = v.init_list(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    ExternalClients::build_value(list.reborrow().get(i as u32), t)
                }
            } // Literal::Policy(p) => v.set_text(p.to_string().as_str()),
        }
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
                    tuple.push(ExternalClients::read_value(t)?)
                }
                Ok(Literal::Tuple(tuple))
            }
            Ok(Which::List(ts)) => {
                let mut list = Vec::new();
                for t in ts? {
                    list.push(ExternalClients::read_value(t)?)
                }
                Ok(Literal::List(list))
            }
            Err(e) => Err(capnp::Error::from(e)),
        }
    }
    pub fn call(&self, call: Call) -> Box<dyn Future<Item = Expr, Error = expressions::Error>> {
        if let Some(client) = self.clients.get(call.external) {
            // prepare the RPC
            let mut req = client.call_request();
            let mut call_builder = req.get();
            // set the name
            call_builder.set_name(call.method);
            // set the args
            let mut call_args = call_builder.init_args(call.args.len() as u32);
            for (i, lit) in call.args.iter().enumerate() {
                ExternalClients::build_value(call_args.reborrow().get(i as u32), lit)
            }
            // let req = ExternalClients::build_request(&call, client);
            Box::new(
                // make the RPC and turn the result into a promise
                req.send()
                    .promise
                    .and_then(move |response| {
                        match ExternalClients::read_value(pry!(pry!(response.get()).get_result())) {
                            Ok(lit) => Promise::ok(lit.into()),
                            Err(err) => Promise::err(err),
                        }
                    })
                    .timeout(call.timeout)
                    .or_else(|err| Promise::err(err.into())),
            )
        } else {
            Box::new(future::err(
                format!("failed to get external: {}", call.external).into(),
            ))
        }
    }
}

impl From<tokio_timer::timeout::Error<capnp::Error>> for expressions::Error {
    fn from(err: tokio_timer::timeout::Error<capnp::Error>) -> expressions::Error {
        err.into_inner()
            .map(|e: capnp::Error| e.to_string().into())
            .unwrap_or_else(|| "timeout error".to_string().into())
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Externals {
    /// map from external names to TCP/Unix socket names
    externals: BTreeMap<String, String>,
    /// time limit and external calls
    timeout: Duration,
}

const TIMEOUT: Duration = Duration::from_secs(3);

impl Default for Externals {
    fn default() -> Self {
        Externals {
            externals: BTreeMap::new(),
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
}
