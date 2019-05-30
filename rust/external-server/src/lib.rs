// For tokio TLS examples see: https://github.com/tokio-rs/tokio/tree/master/tokio-tls/examples
extern crate capnp;
#[macro_use]
extern crate capnp_rpc;

pub mod external_capnp {
    include!(concat!(env!("OUT_DIR"), "/external_capnp.rs"));
}

use crate::external_capnp::external;
use capnp::capability::Promise;
use capnp::Error;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{Future, Stream};
use std::net::ToSocketAddrs;
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;

#[derive(Debug)]
pub enum Literal<'a> {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    DataLiteral(&'a [u8]),
    StringLiteral(&'a str),
    StringList(Vec<&'a str>),
    StringPairs(Vec<(&'a str, &'a str)>),
    Unit,
}

use Literal::{
    BoolLiteral, DataLiteral, FloatLiteral, IntLiteral, StringList, StringLiteral, StringPairs,
};

pub trait Dispatcher {
    fn dispatch<'a>(&'a mut self, name: &str, args: &[Literal<'a>]) -> Result<Literal<'a>, Error>;
    fn process_args<'a>(
        &self,
        args: capnp::struct_list::Reader<'a, external_capnp::external::value::Owned>,
    ) -> Result<Vec<Literal<'a>>, Error> {
        use external::value::Which::{Bool, Data, Float64, Int64, Lines, Pairs, Text, Unit};
        let mut res = Vec::new();
        for arg in args {
            res.push(match arg.which()? {
                Bool(b) => BoolLiteral(b),
                Int64(i) => IntLiteral(i),
                Float64(f) => FloatLiteral(f),
                Text(t) => StringLiteral(t?),
                Data(d) => DataLiteral(d?),
                Unit(_) => Literal::Unit,
                Lines(ps) => {
                    let mut v = Vec::new();
                    for p in ps? {
                        v.push(p?)
                    }
                    StringList(v)
                }
                Pairs(ps) => {
                    let mut v = Vec::new();
                    for p in ps? {
                        v.push((p.get_key()?, p.get_value()?))
                    }
                    StringPairs(v)
                }
            })
        }
        Ok(res)
    }
}

impl<D: Dispatcher> external::Server for D {
    fn call(
        &mut self,
        call: external::CallParams,
        mut result: external::CallResults,
    ) -> Promise<(), Error> {
        // process and print call
        let call = pry!(call.get());
        let name = pry!(call.get_name());
        println!("Call to method: {}", name);
        let args = pry!(self.process_args(pry!(call.get_args())));
        for arg in args.iter() {
            println!("{:?}", arg)
        }
        println!();
        // dispatch to method implementation and then set the result
        let mut res = result.get().init_result();
        Promise::ok(match pry!(self.dispatch(name, &args)) {
            BoolLiteral(b) => res.set_bool(b),
            IntLiteral(i) => res.set_int64(i),
            FloatLiteral(f) => res.set_float64(f),
            StringLiteral(s) => res.set_text(s),
            DataLiteral(d) => res.set_data(d),
            Literal::Unit => res.set_unit(()),
            StringList(ps) => {
                let mut lines = res.init_lines(ps.len() as u32);
                for (i, line) in ps.iter().enumerate() {
                    lines.set(i as u32, line);
                }
            }
            StringPairs(ps) => {
                let mut pairs = res.init_pairs(ps.len() as u32);
                for (i, (key, value)) in ps.iter().enumerate() {
                    let mut pair = pairs.reborrow().get(i as u32);
                    pair.set_key(key);
                    pair.set_value(value)
                }
            }
        })
    }
}

#[cfg(not(target_env = "musl"))]
fn tls_rpc_future(
    socket: tokio::net::TcpListener,
    external: external::Client,
) -> Result<Box<dyn Future<Item = (), Error = std::io::Error>>, Error> {
    // Create the TLS acceptor.
    let cert = native_tls::Identity::from_pkcs12(
        include_bytes!("certificates/server.p12"),
        "rsh-sec-armour",
    )
    .map_err(|err| Error::failed(format!("{}", err)))?;
    let tls_acceptor = tokio_tls::TlsAcceptor::from(
        native_tls::TlsAcceptor::builder(cert)
            .build()
            .map_err(|err| Error::failed(format!("{}", err)))?,
    );
    let fut = socket.incoming().for_each(move |sock| {
        sock.set_nodelay(true)?;
        let external = external.clone();
        let tls_accept = tls_acceptor.accept(sock).and_then(move |tls| {
            let (reader, writer) = tls.split();
            let network = twoparty::VatNetwork::new(
                reader,
                writer,
                rpc_twoparty_capnp::Side::Server,
                Default::default(),
            );
            let rpc_system = RpcSystem::new(Box::new(network), Some(external.client));
            Ok(current_thread::spawn(
                rpc_system.map_err(|e| println!("error: {:?}", e)),
            ))
        });
        println!("new TLS connection");
        Ok(current_thread::spawn(
            tls_accept.map_err(|e| println!("error: {:?}", e)),
        ))
    });
    Ok(Box::new(fut))
}

#[cfg(target_env = "musl")]
fn tls_rpc_future(
    socket: tokio::net::TcpListener,
    external: external::Client,
) -> Result<Box<dyn Future<Item = (), Error = std::io::Error>>, Error> {
    tcp_rpc_future(socket, external)
}

#[cfg(target_env = "musl")]
fn tcp_rpc_future(
    socket: tokio::net::TcpListener,
    external: external::Client,
) -> Result<Box<dyn Future<Item = (), Error = std::io::Error>>, Error> {
    let fut = socket.incoming().for_each(move |socket| {
        socket.set_nodelay(true)?;
        let (reader, writer) = socket.split();
        let network = twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );
        let rpc_system = RpcSystem::new(Box::new(network), Some(external.clone().client));
        println!("WARNING: new insecure connection");
        current_thread::spawn(rpc_system.map_err(|e| println!("error: {:?}", e)));
        Ok(())
    });
    Ok(Box::new(fut))
}

fn uds_rpc_future(
    socket: tokio_uds::UnixListener,
    external: external::Client,
) -> Result<Box<dyn Future<Item = (), Error = std::io::Error>>, Error> {
    let fut = socket.incoming().for_each(move |socket| {
        let (reader, writer) = socket.split();
        let network = twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );
        let rpc_system = RpcSystem::new(Box::new(network), Some(external.clone().client));
        println!("Unix socket connection");
        current_thread::spawn(rpc_system.map_err(|e| println!("error: {:?}", e)));
        Ok(())
    });
    Ok(Box::new(fut))
}

static SOCKET_ERROR: &str = "could not obtain socket address";

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

pub struct External;

impl External {
    pub fn start<
        S: std::net::ToSocketAddrs,
        P: AsRef<std::path::Path> + std::fmt::Display + Clone,
        T: PathOrToSocketAddrs<S, P>,
        U: Dispatcher + 'static,
    >(
        t: T,
        implementation: U,
    ) -> Result<(), Error> {
        let mut fut;
        let external = external::ToClient::new(implementation).into_client::<capnp_rpc::Server>();
        if let Some(s) = t.get_to_socket_addrs() {
            let addr = s
                .to_socket_addrs()
                .map_err(|_| Error::failed(SOCKET_ERROR.to_string()))?
                .next()
                .ok_or(Error::failed(SOCKET_ERROR.to_string()))?;
            let socket = tokio::net::TcpListener::bind(&addr)
                .map_err(|err| Error::failed(format!("{}", err)))?;
            fut = tls_rpc_future(socket, external)?;
        } else if let Some(p) = t.get_path() {
            if p.as_ref().exists() {
                println!("removing old \"{}\"", p);
                std::fs::remove_file(p.clone())?
            };
            let listener = tokio_uds::UnixListener::bind(p)?;
            fut = uds_rpc_future(listener, external)?;
        } else {
            return Err(Error::failed("could not bind".to_string()));
        };

        Ok(current_thread::block_on_all(fut).unwrap())
    }
}
