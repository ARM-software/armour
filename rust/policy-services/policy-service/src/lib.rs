#[macro_use]
extern crate capnp_rpc;

use actix::prelude::*;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use external_capnp::external::ToClient;
use futures::Future;
use log::*;
use tokio::io::AsyncRead;

pub mod external_capnp {
    include!(concat!(env!("OUT_DIR"), "/external_capnp.rs"));
}

pub mod rpc;

pub fn start_policy_service<S: rpc::Dispatcher + 'static>(
    service: S,
    socket: std::path::PathBuf,
) -> std::io::Result<Addr<PolicyService>> {
    let external = ToClient::new(service).into_client::<capnp_rpc::Server>();
    let listener = tokio_uds::UnixListener::bind(&socket)?;
    log::info!(r#"starting policy service at "{}""#, socket.display());
    Ok(PolicyService::create(|ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(Connect));
        PolicyService { socket, external }
    }))
}

pub struct PolicyService {
    socket: std::path::PathBuf,
    external: external_capnp::external::Client,
}

impl Actor for PolicyService {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("removing: {}", self.socket.display());
        std::fs::remove_file(self.socket.clone())
            .unwrap_or_else(|e| warn!("failed to remove: {}", e))
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

struct Connect(tokio_uds::UnixStream);

impl Message for Connect {
    type Result = ();
}

impl Handler<Connect> for PolicyService {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        // For each incoming connection we create `PolicyServiceInstance` actor
        let (reader, writer) = msg.0.split();
        let network = twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );
        let rpc_system = RpcSystem::new(Box::new(network), Some(self.external.clone().client));
        let instance = PolicyServiceInstance::create(move |_ctx| PolicyServiceInstance);
        debug!("starting RPC connection");
        actix::spawn(
            rpc_system
                .map_err(|e| warn!("error: {:?}", e))
                .then(move |x| {
                    instance.do_send(Stop);
                    x
                }),
        );
    }
}

struct PolicyServiceInstance;

impl Actor for PolicyServiceInstance {
    type Context = Context<Self>;

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        debug!("stopped RPC connection")
    }
}

#[derive(Message)]
struct Stop;

impl Handler<Stop> for PolicyServiceInstance {
    type Result = ();
    fn handle(&mut self, _msg: Stop, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop()
    }
}
