use tokio::runtime::current_thread::TaskExecutor;
use futures::{Future, Stream};

use capnp::capability::Promise;

extern crate docker_lib;
use docker_lib::docker_capnp::docker;

use controlplane_lib::controlplane_capnp::control_plane_proto;

extern crate shiplift;
use shiplift::Docker;
use shiplift::EventsOptions;
use shiplift::builder::{EventFilter,
                        EventFilterType,
                        NetworkCreateOptions,
                        ContainerConnectionOptions,
                        ContainerListOptionsBuilder,
                        ContainerFilter};

use tokio_uds::UnixStream;
use tokio::io::AsyncRead;

use capnp_rpc::{RpcSystem, twoparty, rpc_twoparty_capnp};

pub struct DockerImpl;
impl docker::Server for DockerImpl {

    // Requests the deamon to start listening for docker events. 
    // listen @0 () -> ();
    fn listen(&mut self, _: docker::ListenParams, _: docker::ListenResults) -> 
        Promise<(), ::capnp::Error>
    { 
        println!("listening for events");
        let docker = Docker::new();
        let evopt = EventsOptions::builder().filter({ 
            let mut filter = Vec::new();
            // Listens only for container events
            filter.push(EventFilter::Type(EventFilterType::Container));
            filter
        }).build();

        let events_future = docker.events(&evopt);
        let server_future = events_future
            .for_each(|e| {
                let call_future = UnixStream::connect("/Users/guspet02/Downloads/cp".to_string())
                    .map(move |stream| {
                        println!("stream {:?}", stream);
                        let (reader, writer) = stream.split();
                        let network =
                            Box::new(twoparty::VatNetwork::new(reader, std::io::BufWriter::new(writer),
                                                               rpc_twoparty_capnp::Side::Client,
                                                               Default::default()));
                        let mut rpc_system = RpcSystem::new(network, None);
                        let cpp : control_plane_proto::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
                        let mut event = cpp.receive_docker_event_request();
                        event.get()
                            .set_event(&format!("{:?}", e));
                        let _ = TaskExecutor::current().spawn_local(Box::new(
                            rpc_system.join(event.send().promise)
                                .map(|_| ())
                                .map_err(|e| println!("Error {:?}", e))
                        ));
                    })
                    .map_err(|_| ());

                let _ = TaskExecutor::current().spawn_local(Box::new(call_future));
                futures::future::ok(())
            })
            .map(|_| ())
            .map_err(|_| ());

        // This needs to launched as a taks, otherwise capnp event loop gets blocked
        tokio::runtime::current_thread::spawn(Box::new(server_future));
        Promise::ok(())
    }

    // createNetwork @1 (network: Text) -> (result: Bool);
    fn create_network (&mut self, params: docker::CreateNetworkParams, mut results: docker::CreateNetworkResults) -> 
        Promise<(), ::capnp::Error>
    {
        let docker = Docker::new();
        let net_name = pry!(params.get()).get_network().unwrap().to_string();
        println!("creating network {}", net_name);
        results.get().set_result(false);

        let fut = docker
            .networks().create(
                &NetworkCreateOptions::builder(net_name.as_ref())
                    .driver("bridge")
                    .build(),
            )
            .map(move |_| results.get().set_result(true))
            .map_err(|e| capnp::Error::failed(e.to_string()));

        Promise::from_future(fut)
    }

    // removeNetwork @2 (network: Text) -> (result: Bool);
    fn remove_network (&mut self, params: docker::RemoveNetworkParams, mut results: docker::RemoveNetworkResults) -> 
        Promise<(), ::capnp::Error>
    {
        let docker = Docker::new();
        let net_name = pry!(params.get()).get_network().unwrap().to_string();      
        println!("deleting network {}", net_name);
        results.get().set_result(false);
        
        let fut = 
            docker.networks().list(&Default::default())
            .map(|mut networks| {
                networks.retain(move |n| n.name == net_name);
                networks
            })
            .map_err(|e| e.to_string())
            .and_then(|networks| {
                if networks.len() == 1 {
                    futures::future::ok(networks)
                } else { 
                    futures::future::err("network  ambiguous".to_string()) 
                }
            })
            .and_then(move |networks| {
                docker.networks().get(&networks[0].id).delete()
                    .map_err(|e| e.to_string())
            })
            .map(move |_| results.get().set_result(true))
            .map_err(|e| capnp::Error::failed(e.to_string()));

        Promise::from_future(fut)
    }

    // attachToNetwork @3 (container: Text, network: Text) -> (result: Bool)
    fn attach_to_network (&mut self, params: docker::AttachToNetworkParams, mut results: docker::AttachToNetworkResults) -> 
        Promise<(), ::capnp::Error>
    {
        let docker = Docker::new();
        let params = pry!(params.get());
        let cont_name = params.get_container().unwrap().to_string();
        let net_name = params.get_network().unwrap().to_string();      
        println!("attaching {} to network {}", cont_name, net_name);
        results.get().set_result(false);

        let fut = 
            docker.networks().list(&Default::default())
            .map(move |mut networks| {
                networks.retain(|n| n.name == net_name);
                networks
            })
            .map_err(|e| e.to_string())
            .join({
                let mut filter = Vec::new();
                filter.push(ContainerFilter::LabelName(cont_name.to_string()));
                docker.containers().list(&ContainerListOptionsBuilder::default().filter(filter).build())
                    .map_err(|e| e.to_string())
            })
            .and_then(|(networks, containers)| {
                if networks.len() == 1 && containers.len() == 1 {
                    futures::future::ok((networks, containers))
                } else { 
                    futures::future::err("network or container ambiguous".to_string()) 
                }
            })
            .and_then(move |(networks, containers)| {
                docker.networks().get(&networks[0].id)
                    .connect(&ContainerConnectionOptions::builder(&containers[0].id).build())
                    .map_err(|e| e.to_string())
            }
            )
            .map(move |_| results.get().set_result(true))
            .map_err(|e| capnp::Error::failed(e));

        Promise::from_future(fut)
    }

    // detachFromNetwork @4 (container: Text, network: Text) -> (result: Bool);
    fn detach_from_network (&mut self, params: docker::DetachFromNetworkParams, mut results: docker::DetachFromNetworkResults) -> 
        Promise<(), ::capnp::Error>
    {
        let docker = Docker::new();
        let params = pry!(params.get());
        let cont_name = params.get_container().unwrap().to_string();
        let net_name = params.get_network().unwrap().to_string();      
        println!("detaching {} from network {}", cont_name, net_name);
        results.get().set_result(false);

        let fut = 
            docker.networks().list(&Default::default())
            .map(move |mut networks| {
                networks.retain(|n| n.name == net_name);
                networks
            })
            .map_err(|e| e.to_string())
            .join({
                let mut filter = Vec::new();
                filter.push(ContainerFilter::LabelName(cont_name.to_string()));
                docker.containers().list(&ContainerListOptionsBuilder::default().filter(filter).build())
                    .map_err(|e| e.to_string())
            })
            .and_then(|(networks, containers)| {
                if networks.len() == 1 && containers.len() == 1 {
                    futures::future::ok((networks, containers))
                } else { 
                    futures::future::err("network or container ambiguous".to_string()) 
                }
            })
            .and_then(move |(networks, containers)| {
                docker.networks().get(&networks[0].id)
                    .disconnect(&ContainerConnectionOptions::builder(&containers[0].id).build())
                    .map_err(|e| e.to_string())
            }
            )
            .map(move |_| results.get().set_result(true))
            .map_err(|e| capnp::Error::failed(e));

        Promise::from_future(fut)
    }
}
