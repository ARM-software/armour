use capnp::capability::Promise;
use futures::{future, Future, Stream};
use tokio::runtime::current_thread::TaskExecutor;

use controlplane_lib;
use controlplane_lib::controlplane_capnp::control_plane_proto;
use controlplane_lib::controlplane_capnp::control_plane_proto::*;

pub struct ControlPlaneProtoImpl;

impl control_plane_proto::Server for ControlPlaneProtoImpl {
    fn receive_docker_event(
        &mut self,
        params: ReceiveDockerEventParams,
        _: ReceiveDockerEventResults,
    ) -> ::capnp::capability::Promise<(), ::capnp::Error> {
        let ev = pry!(params.get()).get_event().unwrap();
        let fut = future::ok({
            println!("Received the event\n{}", ev.to_string());
        });

        Promise::from_future(fut)
    }
}
