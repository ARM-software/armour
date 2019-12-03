use actix::*;
use tokio;
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_io::codec::{Decoder, Encoder};
use tokio_io::{io::WriteHalf,AsyncRead};
use tokio_uds;
use futures::{Future, stream::Stream};
use tokio_codec::FramedRead;
use std::io::{Error, ErrorKind};

use shiplift::builder::{EventsOptions,
                        EventFilter,
                        EventFilterType,
                        NetworkCreateOptions,
                        ContainerConnectionOptions,
                        ContainerListOptionsBuilder,
                        ContainerFilter};

#[macro_use]
extern crate log;
extern crate env_logger;


#[derive(Message)]
pub struct UdsConnect(pub tokio_uds::UnixStream);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DockerCommands {
    Listen            {socket_name: String}, 
    CreateNetwork     {network: String},
    DeleteNetwork     {network: String},
    AttachToNetwork   {container: String, network: String},
    DetachFromNetwork {container: String, network: String},
}

impl Message for DockerCommands {
    type Result = Result<(), std::io::Error>;
}

pub struct DockerActor {
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, DockerCodec>,
    pub docker: shiplift::Docker,
}

impl DockerActor {
    fn get_container(&mut self, container: String) ->
    impl Future<Item = Result<String, String>, Error=Error> {
        futures::future::ok(Ok(self.docker.containers().get(&container).id().to_string()))
    }

    fn get_network(&mut self, network: String) ->
    impl Future<Item = Result<String, String>, Error=Error> {
        futures::future::ok(Ok(self.docker.networks().get(&network).id().to_string()))
    }
}

impl Actor for DockerActor { 
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        info!("Docker actor started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Docker actor terminated");
    }
}

impl Handler<DockerCommands> for DockerActor {
    type Result = Box<futures::Future<Item=(), Error=Error>>;

    fn handle(&mut self, comm: DockerCommands, _ctx: &mut Self::Context) -> Self::Result {
        match &comm {
            DockerCommands::Listen{ socket_name } => {
                info!("listening for events");

                let evopt = EventsOptions::builder().filter({ 
                    let mut filter = Vec::new();
                    // Listens only for container events
                    filter.push(EventFilter::Type(EventFilterType::Container));
                    filter.push(EventFilter::Type(EventFilterType::Network));
                    filter
                }).build();

                let cp = DockerControlPlaneClient::create_docker_controlplane_client(&socket_name);
             
                let docker_clone = self.docker.clone();
                let event_handler = cp
                    .and_then(|addr| {println!("Connected to Control Plane"); futures::future::ok(addr)})
                    .and_then(move |addr| {
                        docker_clone.events(&evopt)
                            .map_err(|e| Error::new(ErrorKind::Other, format!("Connection error {:?}", e)))
                            .for_each(move |ev| {
                                // info!("{:?}", ev);
                                addr.do_send(DockerEvents {ev : ev});
                                futures::future::ok(())
                            })
                    });
                
                Box::new(event_handler)
            }

            DockerCommands::CreateNetwork{ network } => {
                info!("Creating network {:?}", network);
                let fut = self.docker.networks().create(
                    &NetworkCreateOptions::builder(network.as_ref())
                        .driver("bridge")
                        .build())
                    .map(|_| ())
                    .map_err(|e| {
                        info!("Error {:?}", e);
                        Error::new(ErrorKind::Other, format!("Error creating network {:?}", e))
                    });
                Box::new(fut)
            }

            DockerCommands::DeleteNetwork{ network } => {
                info!("Deleting network {:?}", network);
                let docker_c = self.docker.clone();
                let fut = self.get_network(network.to_string())
                    .map_err(|e| {
                        info!("Error {:?}", e);
                    })
                    .and_then(move |net| {
                        docker_c.networks().get(&net.unwrap()).delete()
                            .map_err(|e| info!("Error {:?}", e))
                    })
                    .map_err(|e| {
                        info!("Error {:?}", e);
                        Error::new(ErrorKind::Other, format!("Error deleting network {:?}", e))
                    });
                Box::new(fut)
            }
            
            DockerCommands::AttachToNetwork{ container, network } => {
                info!("Attaching container {:?} to network {:?}", container, network);
                let docker_c = self.docker.clone();
                let net_clone = network.clone();
                let cont_clone = container.clone();
                let fut =
                    self.get_network(net_clone)
                    .join(self.get_container(cont_clone))
                    .map_err(|e| {
                        info!("Error {:?}", e);
                    })
                    .and_then(move |(network, container)| {
                        docker_c.networks().get(&network.unwrap())
                            .connect(&ContainerConnectionOptions::builder(&container.unwrap()).build())
                            .map_err(|e| info!("Attaching error {:?}", e))
                    })
                    .map_err(|e| {
                        info!("Error {:?}", e);
                        Error::new(ErrorKind::Other, format!("Error attaching network {:?}", e))
                    });
                Box::new(fut)
            }
            
            DockerCommands::DetachFromNetwork{ container, network } => {
                info!("Detaching container {:?} to network {:?}", container, network);
                let docker_c = self.docker.clone();
                let net_clone = network.clone();
                let cont_clone = container.clone();
                let fut =
                    self.get_network(net_clone)
                    .join(self.get_container(cont_clone))
                    .map_err(|e| {
                        info!("Error {:?}", e);
                    })
                    .and_then(move |(network, container)| {
                        docker_c.networks().get(&network.unwrap())
                            .disconnect(&ContainerConnectionOptions::builder(&container.unwrap()).build())
                            .map_err(|e| info!("Error {:?}", e))
                    })
                    .map_err(|e| {
                        info!("Error {:?}", e);
                        Error::new(ErrorKind::Other, format!("Error detaching network {:?}", e))
                    });
                Box::new(fut)
            }
        }
    }    
}

impl StreamHandler<DockerCommands, std::io::Error> for DockerActor {    
    fn handle(&mut self, comm: DockerCommands, ctx: &mut Self::Context) {
        ctx.notify(comm.clone());
    }
}

impl actix::io::WriteHandler<std::io::Error> for DockerActor {}

pub struct DockerCodec;

impl DeserializeDecoder<DockerCommands, std::io::Error> for DockerCodec {}
impl SerializeEncoder<DockerCommands, std::io::Error> for DockerCodec {}

impl Decoder for DockerCodec {
    type Item = DockerCommands;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for DockerCodec {
    type Item = DockerCommands;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}

trait DeserializeDecoder<T: serde::de::DeserializeOwned, E: std::convert::From<std::io::Error>> {
    fn deserialize_decode(&mut self, src: &mut BytesMut) -> Result<Option<T>, E> {
        let size = {
            if src.len() < 2 {
                return Ok(None);
            }
            BigEndian::read_u16(src.as_ref()) as usize
        };
        if src.len() >= size + 2 {
            src.split_to(2);
            let buf = src.split_to(size);
            Ok(Some(bincode::deserialize::<T>(&buf).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })?))
        } else {
            Ok(None)
        }
    }
}

trait SerializeEncoder<T: serde::Serialize, E: std::convert::From<std::io::Error>> {
    fn serialize_encode(&mut self, msg: T, dst: &mut BytesMut) -> Result<(), E> {
        let msg = bincode::serialize(&msg).unwrap();
        let msg_ref: &[u8] = msg.as_ref();
        dst.reserve(msg_ref.len() + 2);
        dst.put_u16_be(msg_ref.len() as u16);
        dst.put(msg_ref);
        Ok(())
    }
}

// Client actor
pub struct DockerClientActor {
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, DockerCodec>,
}

pub struct Unitcodec;

impl DeserializeDecoder<(), std::io::Error> for Unitcodec {}
impl SerializeEncoder<(), std::io::Error> for Unitcodec {}

impl Decoder for Unitcodec {
    type Item = ();
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for Unitcodec {
    type Item = ();
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}

impl DockerClientActor {
    pub fn create_docker_client(socket: String) -> Result<Addr<DockerClientActor>,Error> {
        tokio_uds::UnixStream::connect(socket.to_string())
            .and_then(|stream| {
                let addr = DockerClientActor::create(|ctx| {
                    let (r, w) = stream.split();
                    ctx.add_stream(FramedRead::new(r, Unitcodec));
                    DockerClientActor {
                        uds_framed: actix::io::FramedWrite::new(w, DockerCodec, ctx),
                    }
                } );
                futures::future::ok(addr)
            } )
            .wait()
    }
}

impl StreamHandler<(), std::io::Error> for DockerClientActor {
    fn handle(&mut self, _: (), _ctx: &mut Self::Context) {
        info!("Got a response")
    }
}

impl Actor for DockerClientActor { 
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("Docker client started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Docker client terminated");
    }
}


// simple command repeater
impl Handler<DockerCommands> for DockerClientActor {
    type Result = Result<(), std::io::Error>;

    fn handle(&mut self, command: DockerCommands, _ctx: &mut Context<Self>) -> Self::Result {
        Ok(self.uds_framed.write(command))
    }
}

impl actix::io::WriteHandler<tokio::io::Error> for DockerClientActor {}

// Control plane listener actor

#[derive(Serialize, Deserialize, Clone, Debug, Message)]
pub struct DockerEvents {
    ev: shiplift::rep::Event,
}

pub struct DockerEventCodec;

pub struct DockerControlPlane {
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, DockerEventCodec>,
}

impl DeserializeDecoder<DockerEvents, std::io::Error> for DockerEventCodec {}
impl SerializeEncoder<DockerEvents, std::io::Error> for DockerEventCodec {}

impl Decoder for DockerEventCodec {
    type Item = DockerEvents;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        self.deserialize_decode(src)
    }
}

impl Encoder for DockerEventCodec {
    type Item = DockerEvents;
    type Error = std::io::Error;
    fn encode(&mut self, msg: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.serialize_encode(msg, dst)
    }
}

impl Actor for DockerControlPlane { 
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("Docker control plane started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Docker control plane terminated");
    }
}

impl StreamHandler<DockerEvents, std::io::Error> for DockerControlPlane {    
    fn handle(&mut self, ev: DockerEvents, ctx: &mut Self::Context) {
        ctx.notify(ev.clone());
    }
}

impl StreamHandler<(), std::io::Error> for DockerControlPlane {
    fn handle(&mut self, _: (), _ctx: &mut Self::Context) {
        info!("Got a response")
    }
}

// Simple command repeater
impl Handler<DockerEvents> for DockerControlPlane {
    type Result = ();

    fn handle(&mut self, event: DockerEvents, _ctx: &mut Context<Self>) {
        println!("Received event {:?}", event.ev);
        ()
    }
}

impl actix::io::WriteHandler<tokio::io::Error> for DockerControlPlane {}

struct DockerControlPlaneClient {
    pub uds_framed: actix::io::FramedWrite<WriteHalf<tokio_uds::UnixStream>, DockerEventCodec>,
}

impl DockerControlPlaneClient {
    pub fn create_docker_controlplane_client(socket: &str) -> impl Future<Item=Box<Addr<DockerControlPlaneClient>>, Error=Error> {
        tokio_uds::UnixStream::connect(socket.to_string())
            .and_then(|stream| {
                let addr = DockerControlPlaneClient::create(|ctx| {
                    let (r, w) = stream.split();
                    ctx.add_stream(FramedRead::new(r, Unitcodec));
                    DockerControlPlaneClient {
                        uds_framed: actix::io::FramedWrite::new(w, DockerEventCodec, ctx),
                    }
                });
                futures::future::ok(Box::new(addr))
            })
    }
}

impl StreamHandler<(), std::io::Error> for DockerControlPlaneClient {
    fn handle(&mut self, _: (), _ctx: &mut Self::Context) {
        info!("Got a response")
    }
}

impl Actor for DockerControlPlaneClient { 
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("Docker client started");
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        info!("Docker client terminated");
    }
}

// simple command repeater
impl Handler<DockerEvents> for DockerControlPlaneClient {
    type Result = ();

    fn handle(&mut self, ev: DockerEvents, _ctx: &mut Context<Self>) -> Self::Result {
        self.uds_framed.write(ev);
    }
}

impl actix::io::WriteHandler<tokio::io::Error> for DockerControlPlaneClient {}
