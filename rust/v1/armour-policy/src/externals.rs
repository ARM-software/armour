use super::expressions::{self, Expr};
use super::lang::Program;
use super::literals::Literal;
use crate::external_capnp::external;
use actix::prelude::*;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{future::FutureExt, Future};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

// TODO: move more into Actor to avoid work
pub struct ExternalActor {
    pub externals: Arc<Externals>,
}

impl Actor for ExternalActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<Expr, expressions::Error>")]
pub struct Call {
    external: String,
    method: String,
    args: Vec<Literal>,
}

impl Call {
    pub fn new(external: &str, method: &str, args: Vec<Literal>) -> Self {
        Call {
            external: external.to_string(),
            method: method.to_string(),
            args,
        }
    }
}

impl Handler<Call> for ExternalActor {
    type Result = ResponseFuture<Result<Expr, expressions::Error>>;
    fn handle(&mut self, call: Call, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(Externals::call(self.externals.clone(), call))
    }
}

pub type Disconnector = Box<dyn Future<Output = ()> + std::marker::Unpin>;

impl ExternalActor {
    pub fn new(prog: Arc<Program>) -> Self {
        ExternalActor {
            externals: Arc::new(prog.externals.clone()),
        }
    }
}

impl From<capnp::Error> for expressions::Error {
    fn from(err: capnp::Error) -> expressions::Error {
        err.to_string().into()
    }
}

impl From<async_std::future::TimeoutError> for expressions::Error {
    fn from(_err: async_std::future::TimeoutError) -> expressions::Error {
        "timeout error".to_string().into()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Externals {
    /// map from external names to TCP/Unix socket names
    sockets: BTreeMap<String, String>,
    /// time limit and external calls
    timeout: Duration,
}

const TIMEOUT: Duration = Duration::from_secs(3);

impl Default for Externals {
    fn default() -> Self {
        Externals {
            sockets: BTreeMap::new(),
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
        self.sockets
            .insert(name.to_string(), addr.to_string())
            .is_some()
    }
    pub async fn client(
        socket: &str,
    ) -> Result<
        (
            external::Client,
            capnp_rpc::Disconnector<rpc_twoparty_capnp::Side>,
        ),
        std::io::Error,
    > {
        if let Ok(mut p) = socket.to_socket_addrs() {
            let stream = async_std::net::TcpStream::connect(&p.next().unwrap()).await?;
            Ok(Externals::get(stream))
        } else {
            let stream = async_std::os::unix::net::UnixStream::connect(socket)
                .await
                .map_err(move |e| std::io::Error::new(e.kind(), format!("{}: {}", socket, e)))?;
            Ok(Externals::get(stream))
            // .map_err(move |e| std::io::Error::new(e.kind(), format!("{}: {}", socket, e))),
        }
    }
    fn get<S>(
        stream: S,
    ) -> (
        external::Client,
        capnp_rpc::Disconnector<rpc_twoparty_capnp::Side>,
    )
    where
        S: futures::io::AsyncReadExt + futures::io::AsyncWrite + 'static + Unpin,
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
        let disconnector = rpc_system.get_disconnector();
        actix::spawn(Box::pin(rpc_system.map(|_| ())));
        (client, disconnector)
    }
    /// Build a Cap'n Proto literal from an Armour literal
    pub fn build_value(mut v: external::value::Builder<'_>, lit: &Literal) {
        match lit {
            Literal::Bool(b) => v.set_bool(*b),
            Literal::Connection(conn) => Externals::build_value(v, &Literal::from(conn)),
            Literal::Data(d) => v.set_data(d),
            Literal::Float(f) => v.set_float64(*f),
            Literal::HttpRequest(req) => Externals::build_value(v, &Literal::from(req)),
            Literal::HttpResponse(res) => Externals::build_value(v, &Literal::from(res)),
            Literal::ID(id) => Externals::build_value(v, &Literal::from(id)),
            Literal::Int(i) => v.set_int64(*i),
            Literal::IpAddr(ip) => Externals::build_value(v, &Literal::from(ip)),
            Literal::Payload(pld) => Externals::build_value(v, &Literal::from(pld)),
            Literal::Regex(r) => v.set_text(&r.to_string()),
            Literal::Str(s) => v.set_text(s),
            Literal::Unit => v.set_unit(()),
            Literal::Tuple(ts) => {
                let mut tuple = v.init_tuple(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(tuple.reborrow().get(i as u32), t)
                }
            }
            Literal::List(ts) => {
                let mut list = v.init_list(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(list.reborrow().get(i as u32), t)
                }
            } // Literal::Policy(p) => v.set_text(p.to_string().as_str()),
        }
    }
    /// Read a Cap'n Proto literal and return an Armour literal
    pub fn read_value(v: external::value::Reader<'_>) -> Result<Literal, capnp::Error> {
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
    pub async fn call(externals: Arc<Externals>, call: Call) -> Result<Expr, expressions::Error> {
        if let Some(socket) = externals.sockets.get(&call.external) {
            let (client, _disconnector) = Externals::client(socket).await?;
            // prepare the RPC
            let mut req = client.call_request();
            let mut call_builder = req.get();
            // set the name
            call_builder.set_name(&call.method);
            // set the args
            let mut call_args = call_builder.init_args(call.args.len() as u32);
            for (i, lit) in call.args.iter().enumerate() {
                Externals::build_value(call_args.reborrow().get(i as u32), lit)
            }
            let response =
                async_std::future::timeout(externals.timeout, req.send().promise).await?;
            // disconnector.await?;
            match Externals::read_value(response?.get()?.get_result()?) {
                Ok(lit) => Ok(lit.into()),
                Err(err) => Err(err.into()),
            }
        } else {
            Err(format!("failed to get external: {}", call.external).into())
        }
    }
}
