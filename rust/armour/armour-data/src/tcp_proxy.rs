use super::{policy, Stop};
use actix::prelude::*;
use bytes::{Bytes, BytesMut};
use futures::{future, Future};
use std::collections::HashSet;
use std::net::SocketAddr;
#[cfg(any(target_os = "linux"))]
use std::os::unix::io::AsRawFd;
use tokio_codec::{BytesCodec, FramedRead};
use tokio_io::{io::WriteHalf, AsyncRead};

pub fn start_proxy(
    proxy_port: u16,
    policy: Addr<policy::DataPolicy>,
) -> std::io::Result<Addr<TcpDataServer>> {
    // start master actor (for keeping track of connections)
    let master = TcpDataMaster::start_default();
    // start server, listening for connections on a TCP socket
    let socket_in = SocketAddr::from(([0, 0, 0, 0], proxy_port));
    log::info!("starting TCP repeater on port {}", proxy_port,);
    let listener = tokio_tcp::TcpListener::bind(&socket_in)?;
    let master_clone = master.clone();
    let server = TcpDataServer::create(move |ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(TcpConnect));
        TcpDataServer {
            master: master_clone,
            policy,
            port: socket_in.port(),
        }
    });
    Ok(server)
}

/// Actor that handles Unix socket connections.
///
/// When new data plane instances arrive, we give them the address of the master.
pub struct TcpDataServer {
    master: Addr<TcpDataMaster>,
    policy: Addr<policy::DataPolicy>,
    pub port: u16,
}

impl Actor for TcpDataServer {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("stopped socket: {}", self.port);
        self.master.do_send(Stop)
    }
}

/// Notification of new TCP socket connection
// #[derive(Message)]
struct TcpConnect(tokio_tcp::TcpStream);

impl Message for TcpConnect {
    type Result = Result<(), ()>;
}

// The following is needed for testing:

// Terminal 1 (for armour-data-master)
// - docker-machine create armour
// - docker-machine ssh armour
// - sudo iptables -t nat -I PREROUTING -i armour -p tcp --dport 443 -j DNAT --to-destination 127.0.0.1:8443
// - sudo sysctl -w net.ipv4.conf.armour.route_localnet=1
// - $TARGET_PATH/armour-data-master

// Terminal 2 (for client)
// - eval `docker-machine env armour`
// - docker network create --subnet 10.0.0.0/28 -o "com.docker.network.bridge.name"="armour" armour
// - docker run -ti --rm --net armour --ip 10.0.0.2 ubuntu
//     - apt update
//     - apt install curl
//     - curl https://...

// obtain the original socket destination (SO_ORIGINAL_DST)
// we assume Linux's `iptables` have been used to redirect connections to the proxy
#[cfg(target_os = "linux")]
fn original_dst(sock: &tokio_tcp::TcpStream) -> Option<std::net::SocketAddr> {
    if let Ok(sock_in) =
        nix::sys::socket::getsockopt(sock.as_raw_fd(), nix::sys::socket::sockopt::OriginalDst)
    {
        // swap byte order
        let (addr, port) = if cfg!(target_endian = "little") {
            (sock_in.sin_addr.s_addr.to_be(), sock_in.sin_port.to_be())
        } else {
            (sock_in.sin_addr.s_addr.to_le(), sock_in.sin_port.to_le())
        };
        let socket = std::net::SocketAddr::from(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::from(addr),
            port,
        ));
        Some(socket)
    // log::debug!("SO_ORIGINAL_DST: {}", socket);
    // None
    } else {
        None
    }
}

#[cfg(not(target_os = "linux"))]
fn original_dst(_sock: &tokio_tcp::TcpStream) -> Option<std::net::SocketAddr> {
    None
}

impl Handler<TcpConnect> for TcpDataServer {
    // type Result = ();
    type Result = Box<dyn Future<Item = (), Error = ()>>;

    fn handle(&mut self, msg: TcpConnect, _: &mut Context<Self>) -> Self::Result {
        // get the orgininal server socket address
        if let Some(socket) = original_dst(&msg.0) {
            if let Ok(peer_addr) = msg.0.peer_addr() {
                info!(
                    "TCP {}: received from {}, forwarding to {}",
                    self.port, peer_addr, socket
                );
                // For each incoming connection we create `TcpDataClientInstance` actor
                // We also create a `TcpDataServerInstance` actor
                let master = self.master.clone();
                if socket.port() == self.port
                    && armour_data_interface::INTERFACE_IPS.contains(&socket.ip())
                {
                    warn!("TCP {}: trying to forward to self", self.port);
                    if let Err(e) = msg.0.shutdown(std::net::Shutdown::Both) {
                        warn!("{}", e);
                    };
                    Box::new(futures::future::ok(()))
                } else {
                    let server = self
                        .policy
                        .send(policy::ConnectPolicy(peer_addr, socket))
                        .then(move |res| match res {
                            // allow connection
                            Ok(Ok(true)) => future::Either::A(
                                tokio_tcp::TcpStream::connect(&socket).and_then(|sock| {
                                    let client = TcpDataClientInstance::create(|ctx| {
                                        let (r, w) = msg.0.split();
                                        TcpDataClientInstance::add_stream(
                                            FramedRead::new(r, BytesCodec::new()),
                                            ctx,
                                        );
                                        TcpDataClientInstance {
                                            server: None,
                                            tcp_framed: actix::io::FramedWrite::new(
                                                w,
                                                BytesCodec::new(),
                                                ctx,
                                            ),
                                        }
                                    });
                                    TcpDataServerInstance::create(|ctx| {
                                        let (r, w) = sock.split();
                                        TcpDataServerInstance::add_stream(
                                            FramedRead::new(r, BytesCodec::new()),
                                            ctx,
                                        );
                                        TcpDataServerInstance {
                                            master,
                                            client,
                                            tcp_framed: actix::io::FramedWrite::new(
                                                w,
                                                BytesCodec::new(),
                                                ctx,
                                            ),
                                        }
                                    });
                                    Ok(())
                                }),
                            ),
                            // reject
                            Ok(Ok(false)) => {
                                info!("connection denied");
                                if let Err(e) = msg.0.shutdown(std::net::Shutdown::Both) {
                                    warn!("{}", e);
                                };
                                future::Either::B(future::ok(()))
                            }
                            // policy error
                            Ok(Err(e)) => {
                                warn!("{}", e);
                                if let Err(e) = msg.0.shutdown(std::net::Shutdown::Both) {
                                    warn!("{}", e);
                                };
                                future::Either::B(future::ok(()))
                            }
                            // actor error
                            Err(e) => {
                                warn!("{}", e);
                                if let Err(e) = msg.0.shutdown(std::net::Shutdown::Both) {
                                    warn!("{}", e);
                                };
                                future::Either::B(future::ok(()))
                            }
                        })
                        .map_err(|err| warn!("{}", err));
                    Box::new(server)
                }
            } else {
                warn!("TCP {}: could not obtain source IP address", self.port);
                Box::new(futures::future::ok(()))
            }
        } else {
            warn!("TCP {}: could not obtain original destination", self.port);
            Box::new(futures::future::ok(()))
        }
    }
}

/// Actor that manages data plane TCP connections
#[derive(Default)]
struct TcpDataMaster {
    connections: HashSet<Addr<TcpDataServerInstance>>,
}

impl Actor for TcpDataMaster {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        for server in self.connections.iter() {
            server.do_send(Stop)
        }
    }
}

/// Connection notification (from Client Instance to Master)
#[derive(Message)]
struct ConnectServer(Addr<TcpDataServerInstance>);

impl Handler<ConnectServer> for TcpDataMaster {
    type Result = ();
    fn handle(&mut self, msg: ConnectServer, _ctx: &mut Context<Self>) -> Self::Result {
        self.connections.insert(msg.0.clone());
    }
}

impl Handler<ConnectServer> for TcpDataClientInstance {
    type Result = ();
    fn handle(&mut self, msg: ConnectServer, _ctx: &mut Context<Self>) -> Self::Result {
        self.server = Some(msg.0)
    }
}

/// Disconnect notification (from Instance to Master)
#[derive(Message)]
struct Disconnect(Addr<TcpDataServerInstance>);

impl Handler<Disconnect> for TcpDataMaster {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) -> Self::Result {
        info!("removing TCP instance");
        self.connections.remove(&msg.0);
    }
}

/// Actor that handles TCP communication with a data plane instance
///
/// There will be one actor per TCP socket connection
struct TcpDataClientInstance {
    server: Option<Addr<TcpDataServerInstance>>,
    tcp_framed: actix::io::FramedWrite<WriteHalf<tokio_tcp::TcpStream>, BytesCodec>,
}

impl Actor for TcpDataClientInstance {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(server) = &self.server {
            server.do_send(Stop);
        }
    }
}

impl actix::io::WriteHandler<std::io::Error> for TcpDataClientInstance {}

// read from client becomes write to server
impl StreamHandler<BytesMut, std::io::Error> for TcpDataClientInstance {
    fn handle(&mut self, msg: BytesMut, _ctx: &mut Self::Context) {
        if let Some(server) = &self.server {
            server.do_send(Write(msg.freeze()))
        } else {
            warn!("no server")
        }
    }
    fn finished(&mut self, ctx: &mut Context<Self>) {
        info!("end of connection");
        ctx.stop()
    }
}

impl Handler<Stop> for TcpDataServerInstance {
    type Result = ();
    fn handle(&mut self, _msg: Stop, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop()
    }
}

impl Handler<Stop> for TcpDataServer {
    type Result = ();
    fn handle(&mut self, _msg: Stop, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop()
    }
}

impl Handler<Stop> for TcpDataMaster {
    type Result = ();
    fn handle(&mut self, _msg: Stop, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop()
    }
}

/// Actor that handles TCP communication with a data plane instance
///
/// There will be one actor per TCP socket connection
struct TcpDataServerInstance {
    master: Addr<TcpDataMaster>,
    client: Addr<TcpDataClientInstance>,
    tcp_framed: actix::io::FramedWrite<WriteHalf<tokio_tcp::TcpStream>, BytesCodec>,
}

impl Actor for TcpDataServerInstance {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.client
            .send(ConnectServer(ctx.address()))
            .into_actor(self)
            .then(|res, _act, ctx| {
                if res.is_err() {
                    ctx.stop()
                };
                actix::fut::ok(())
            })
            .wait(ctx);
        self.master
            .send(ConnectServer(ctx.address()))
            .into_actor(self)
            .then(|res, _act, ctx| {
                if res.is_err() {
                    ctx.stop()
                };
                actix::fut::ok(())
            })
            .wait(ctx)
    }
    fn stopped(&mut self, ctx: &mut Self::Context) {
        self.master.do_send(Disconnect(ctx.address()));
    }
}

impl actix::io::WriteHandler<std::io::Error> for TcpDataServerInstance {}

// read from server becomes write to client
impl StreamHandler<BytesMut, std::io::Error> for TcpDataServerInstance {
    fn handle(&mut self, msg: BytesMut, _ctx: &mut Self::Context) {
        self.client.do_send(Write(msg.freeze()))
    }
}

#[derive(Message)]
struct Write(Bytes);

impl Handler<Write> for TcpDataClientInstance {
    type Result = ();
    fn handle(&mut self, msg: Write, _ctx: &mut Context<Self>) {
        self.tcp_framed.write(msg.0)
    }
}

impl Handler<Write> for TcpDataServerInstance {
    type Result = ();
    fn handle(&mut self, msg: Write, _ctx: &mut Context<Self>) {
        self.tcp_framed.write(msg.0)
    }
}
