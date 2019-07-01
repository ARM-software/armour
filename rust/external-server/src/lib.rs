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
use std::fmt;
use std::net::ToSocketAddrs;
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;

#[derive(Debug, Clone)]
pub enum Literal {
    IntLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    DataLiteral(Vec<u8>),
    StringLiteral(String),
    List(Vec<Literal>),
    Tuple(Vec<Literal>),
    Unit,
}

impl Literal {
    fn is_tuple(&self) -> bool {
        match self {
            Literal::Tuple(_) => true,
            _ => false,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::IntLiteral(i) => write!(f, "{}", i),
            Literal::FloatLiteral(d) => {
                if 8 < d.abs().log10() as usize {
                    write!(f, "{:e}", d)
                } else if d.trunc() == *d {
                    write!(f, "{:.1}", d)
                } else {
                    write!(f, "{}", d)
                }
            }
            Literal::BoolLiteral(b) => write!(f, "{}", b),
            Literal::DataLiteral(d) => write!(f, "{}", String::from_utf8_lossy(d)),
            Literal::StringLiteral(s) => write!(f, r#""{}""#, s),
            Literal::List(lits) | Literal::Tuple(lits) => {
                let s = lits
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                if self.is_tuple() {
                    write!(f, "({})", s)
                } else {
                    write!(f, "[{}]", s)
                }
            }
            Literal::Unit => write!(f, "()"),
        }
    }
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
                build_value(tuple.reborrow().get(i as u32), t)?
            }
        }
        Literal::List(ts) => {
            let mut list = v.init_list(ts.len() as u32);
            for (i, t) in ts.iter().enumerate() {
                build_value(list.reborrow().get(i as u32), t)?
            }

        }
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
                tuple.push(read_value(t)?)
            }
            Ok(Literal::Tuple(tuple))
        }
        Ok(Which::List(ts)) => {
            let mut list = Vec::new();
            for t in ts? {
                list.push(read_value(t)?)
            }
            Ok(Literal::List(list))
        }
        Err(e) => Err(capnp::Error::from(e)),
    }
}

pub trait Dispatcher {
    fn dispatch(&mut self, name: &str, args: &[Literal]) -> Result<Literal, Error>;
    fn process_args(
        &self,
        args: capnp::struct_list::Reader<external_capnp::external::value::Owned>,
    ) -> Result<Vec<Literal>, Error> {
        let mut res = Vec::new();
        for arg in args {
            res.push(read_value(arg)?)
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
            println!("{}", arg)
        }
        println!();
        // dispatch to method implementation and then set the result
        let res = result.get().init_result();
        if let Err(e) = build_value(res, &pry!(self.dispatch(name, &args))) {
            Promise::err(e)
        } else {
            Promise::ok(())
        }
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
