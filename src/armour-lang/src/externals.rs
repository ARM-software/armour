use super::expressions::{self, Expr, DPExpr};
use super::types::{self, TFlatTyp};
use super::types_cp;
use super::lang::Program;
use super::literals::{self, CPLiteral, CPFlatLiteral, DPLiteral, DPFlatLiteral, Literal, TFlatLiteral};
use crate::external_capnp::external;
use actix::prelude::*;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{future::FutureExt, Future};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;
use std::marker::PhantomData;

pub struct ExternalActor {
    pub externals: Arc<Externals>,
}

impl Actor for ExternalActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<Expr<FlatTyp, FlatLiteral>, expressions::Error>")]
pub struct Call<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+'static> {
    external: String,
    method: String,
    args: Vec<Literal<FlatTyp, FlatLiteral>>,
    phantom: PhantomData<(FlatTyp, FlatLiteral)>
}

macro_rules! dpflatlit (
  ($i: ident ) => (
        Literal::FlatLiteral(DPFlatLiteral::$i)
  );
  ($i: ident ($($args:tt)*) ) => (
        Literal::FlatLiteral(DPFlatLiteral::$i($($args)*))
  );
);
macro_rules! cpflatlit (
  ($i: ident ) => (
        Literal::FlatLiteral(CPFlatLiteral::$i)
  );
  ($i: ident ($($args:tt)*) ) => (
        Literal::FlatLiteral(CPFlatLiteral::$i($($args)*))
  );
);
impl<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>> Call<FlatTyp, FlatLiteral> {
    pub fn new(external: &str, method: &str, args: Vec<Literal<FlatTyp, FlatLiteral>>) -> Self {
        Call {
            external: external.to_string(),
            method: method.to_string(),
            args,
            phantom: PhantomData
        }
    }
    pub fn path(&self) -> String {
        format!("{}::{}", self.external, self.method)
    }
    pub fn split(&self) -> (&str, &str, &[Literal<FlatTyp, FlatLiteral>]) {
        (
            self.external.as_str(),
            self.method.as_str(),
            self.args.as_slice(),
        )
    }
}

impl<FlatTyp:TFlatTyp+'static, FlatLiteral:TFlatLiteral<FlatTyp>+TExternals<FlatTyp, FlatLiteral>+'static> Handler<Call<FlatTyp, FlatLiteral>> for ExternalActor {
    type Result = ResponseFuture<Result<Expr<FlatTyp, FlatLiteral>, expressions::Error>>;
    fn handle(&mut self, call: Call<FlatTyp, FlatLiteral>, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(Externals::call(self.externals.clone(), call))
    }
}

pub type Disconnector = Box<dyn Future<Output = ()> + std::marker::Unpin>;

impl ExternalActor {
    pub fn new<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>(prog: &Program<FlatTyp, FlatLiteral>) -> Self {
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

//To specialize buil/read_value
pub trait TExternals<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>>{
    fn build_value(v: external::value::Builder<'_>, lit: &Literal<FlatTyp, FlatLiteral>);
    //fn read_value(v: external::value::Reader<'_>) -> Result<Literal<FlatTyp, FlatLiteral>, capnp::Error>;

    /// Read a Cap'n Proto literal and return an Armour literal
    fn read_value(v: external::value::Reader<'_>) -> Result<Literal<FlatTyp, FlatLiteral>, capnp::Error> {
        use external::value::Which;
        match v.which() {
            Ok(Which::Bool(b)) => Ok(Literal::bool(b)),
            Ok(Which::Int64(i)) => Ok(Literal::int(i)),
            Ok(Which::Float64(f)) => Ok(Literal::float(f)),
            Ok(Which::Text(t)) => Ok(Literal::str(t?.to_string())),
            Ok(Which::Data(d)) => Ok(Literal::data(d?.to_vec())),
            Ok(Which::Unit(_)) => Ok(Literal::unit()),
            Ok(Which::Tuple(ts)) => {
                let mut tuple = Vec::new();
                for t in ts? {
                    tuple.push(Self::read_value(t)?)
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
}
impl TExternals<types::FlatTyp, literals::DPFlatLiteral> for literals::DPFlatLiteral {
    /// Build a Cap'n Proto literal from an Armour literal
    fn build_value(mut v: external::value::Builder<'_>, lit: &DPLiteral) {
        match lit {
            dpflatlit!(Bool(b)) => v.set_bool(*b),
            dpflatlit!(Connection(conn)) => Externals::build_value(v, &DPLiteral::from(conn)),
            dpflatlit!(Data(d)) => v.set_data(d),
            dpflatlit!(Float(f)) => v.set_float64(*f),
            dpflatlit!(HttpRequest(req)) => Externals::build_value(v, &DPLiteral::from(&**req)),
            dpflatlit!(HttpResponse(res)) => Externals::build_value(v, &DPLiteral::from(&**res)),
            dpflatlit!(ID(id)) => Externals::build_value(v, &DPLiteral::from(id)),
            dpflatlit!(Int(i)) => v.set_int64(*i),
            dpflatlit!(IpAddr(ip)) => Externals::build_value(v, &DPLiteral::from(ip)),
            dpflatlit!(Label(label)) => v.set_text(&label.to_string()),
            dpflatlit!(Regex(r)) => v.set_text(&r.to_string()),
            dpflatlit!(Str(s)) => v.set_text(s),
            dpflatlit!(Unit) => v.set_unit(()),
            DPLiteral::Tuple(ts) => {
                let mut tuple = v.init_tuple(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(tuple.reborrow().get(i as u32), t)
                }
            }
            DPLiteral::List(ts) => {
                let mut list = v.init_list(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(list.reborrow().get(i as u32), t)
                }
            },
            Literal::Phantom(_) => unimplemented!()
        }
    }
}

impl TExternals<types_cp::CPFlatTyp, literals::CPFlatLiteral> for literals::CPFlatLiteral {
    /// Build a Cap'n Proto literal from an Armour literal
    fn build_value(mut v: external::value::Builder<'_>, lit: &CPLiteral) {
        match lit {
            cpflatlit!(Bool(b)) => v.set_bool(*b),
            cpflatlit!(Connection(conn)) => Externals::build_value(v, &CPLiteral::from(conn)),
            cpflatlit!(Data(d)) => v.set_data(d),
            cpflatlit!(Float(f)) => v.set_float64(*f),
            cpflatlit!(HttpRequest(req)) => Externals::build_value(v, &CPLiteral::from(&**req)),
            cpflatlit!(HttpResponse(res)) => Externals::build_value(v, &CPLiteral::from(&**res)),
            cpflatlit!(ID(id)) => Externals::build_value(v, &CPLiteral::from(id)),
            cpflatlit!(Int(i)) => v.set_int64(*i),
            cpflatlit!(IpAddr(ip)) => Externals::build_value(v, &CPLiteral::from(ip)),
            cpflatlit!(Label(label)) => v.set_text(&label.to_string()),
            cpflatlit!(OnboardingData(data)) => Externals::build_value(v, &CPLiteral::from(&**data)),
            cpflatlit!(OnboardingResult(res)) => Externals::build_value(v, &CPLiteral::from(&**res)),
            cpflatlit!(Policy(pol)) => Externals::build_value(v, &CPLiteral::from(&**pol)),
            cpflatlit!(Regex(r)) => v.set_text(&r.to_string()),
            cpflatlit!(Str(s)) => v.set_text(s),
            cpflatlit!(Unit) => v.set_unit(()),
            CPLiteral::Tuple(ts) => {
                let mut tuple = v.init_tuple(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(tuple.reborrow().get(i as u32), t)
                }
            }
            CPLiteral::List(ts) => {
                let mut list = v.init_list(ts.len() as u32);
                for (i, t) in ts.iter().enumerate() {
                    Externals::build_value(list.reborrow().get(i as u32), t)
                }
            },
            Literal::Phantom(_) => unimplemented!()
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
            let pp = p.next().unwrap();
            log::info!("trying to connect to TCP external: {}", pp);
            let stream = async_std::net::TcpStream::connect(&pp).await?;
            log::info!("connected to TCP external");
            Ok(Externals::get(stream))
        } else {
            let stream = async_std::os::unix::net::UnixStream::connect(socket)
                .await
                .map_err(move |e| std::io::Error::new(e.kind(), format!("{}: {}", socket, e)))?;
            Ok(Externals::get(stream))
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
    pub fn build_value<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>+TExternals<FlatTyp, FlatLiteral>>(mut v: external::value::Builder<'_>, lit: &Literal<FlatTyp, FlatLiteral>) {
        FlatLiteral::build_value(v, lit)
    }
    /// Read a Cap'n Proto literal and return an Armour literal
    pub fn read_value<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>+TExternals<FlatTyp, FlatLiteral>>(v: external::value::Reader<'_>) -> Result<Literal<FlatTyp, FlatLiteral>, capnp::Error> {
        FlatLiteral::read_value(v)
    }
    pub async fn call<FlatTyp:TFlatTyp, FlatLiteral:TFlatLiteral<FlatTyp>+TExternals<FlatTyp, FlatLiteral>>(externals: Arc<Externals>, call: Call<FlatTyp, FlatLiteral>) -> Result<Expr<FlatTyp, FlatLiteral>, expressions::Error> {
        if let Some(socket) = externals.sockets.get(&call.external) {
            let (client, disconnector) = Externals::client(socket).await?;
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
            disconnector.await?;
            match Externals::read_value(response?.get()?.get_result()?) {
                Ok(lit) => Ok(lit.into()),
                Err(err) => Err(err.into()),
            }
        } else {
            Err(format!("failed to get external: {}", call.path()).into())
        }
    }
}
