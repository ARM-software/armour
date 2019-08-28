#[macro_use]
extern crate capnp_rpc;

use actix::prelude::*;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use external_capnp::external::ToClient;
use futures::Future;
use log::*;

pub mod external_capnp {
    include!(concat!(env!("OUT_DIR"), "/external_capnp.rs"));
}

pub mod rpc;

pub fn start_uds_policy_service<S: rpc::Dispatcher + 'static>(
    service: S,
    socket: std::path::PathBuf,
) -> std::io::Result<Addr<PolicyService>> {
    let external = ToClient::new(service).into_client::<capnp_rpc::Server>();
    let listener = tokio_uds::UnixListener::bind(&socket)?;
    log::info!(r#"starting UDS policy service at "{}""#, socket.display());
    Ok(PolicyService::create(|ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(Connect));
        PolicyService {
            socket: Some(socket),
            external,
        }
    }))
}

pub fn start_tcp_policy_service<S: rpc::Dispatcher + 'static, A: std::net::ToSocketAddrs>(
    service: S,
    socket: A,
) -> std::io::Result<Addr<PolicyService>> {
    let external = ToClient::new(service).into_client::<capnp_rpc::Server>();
    let addr = socket
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "bad socket address"))?;
    let listener = tokio::net::TcpListener::bind(&addr)?;
    log::info!(r#"starting TCP policy service at "{}""#, addr);
    Ok(PolicyService::create(|ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(Connect));
        PolicyService {
            socket: None,
            external,
        }
    }))
}

pub struct PolicyService {
    socket: Option<std::path::PathBuf>,
    external: external_capnp::external::Client,
}

impl Actor for PolicyService {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        if let Some(socket) = &self.socket {
            info!("removing: {}", socket.display());
            std::fs::remove_file(socket).unwrap_or_else(|e| warn!("failed to remove: {}", e))
        }
    }
}

#[derive(Message)]
pub struct Quit;

impl Handler<Quit> for PolicyService {
    type Result = ();
    fn handle(&mut self, _msg: Quit, _ctx: &mut Context<Self>) -> Self::Result {
        System::current().stop()
    }
}

// struct Connect(tokio_uds::UnixStream);

struct Connect<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Sized + 'static>(T);

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Sized + 'static> Message for Connect<T> {
    type Result = ();
}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Sized + 'static> Handler<Connect<T>>
    for PolicyService
{
    type Result = ();

    fn handle(&mut self, msg: Connect<T>, _: &mut Context<Self>) {
        // For each incoming connection we create `PolicyServiceInstance` actor
        let (reader, writer) = msg.0.split();
        let network = twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );
        let rpc_system = RpcSystem::new(Box::new(network), Some(self.external.clone().client));
        debug!("starting RPC connection");
        actix::spawn(
            rpc_system
                .map_err(|e| warn!("error: {:?}", e))
                .then(move |x| {
                    debug!("stopped RPC connection");
                    x
                }),
        );
    }
}
