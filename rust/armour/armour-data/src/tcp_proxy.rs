// The following is needed for testing:

// Terminal 1 (for armour-data-master)
// - docker-machine create --virtualbox-cpu-count "2" --virtualbox-memory "4096" armour
// - docker-machine ssh armour
// - sudo iptables -t nat -I PREROUTING -i armour -p tcp --dport 443 -j DNAT --to-destination 127.0.0.1:8443
// - sudo iptables -t nat -I PREROUTING -i armour -p tcp --dport 5001 -j DNAT --to-destination 127.0.0.1:6000
// - sudo sysctl -w net.ipv4.conf.armour.route_localnet=1
// - $TARGET_PATH/armour-data-master

// Terminal 2 (for client)
// - eval `docker-machine env armour`
// - docker network create --subnet 10.0.0.0/28 -o "com.docker.network.bridge.name"="armour" armour
// - docker run -ti --rm --net armour --ip 10.0.0.2 ubuntu
//     - apt update
//     - apt install curl
//     - curl https://...

use super::{
    policy,
    tcp_codec::{client, server},
    tcp_policy, Stop,
};
use actix::prelude::*;
use futures::{future, Future};
use policy::PolicyActor;
use std::net::SocketAddr;
use tcp_policy::TcpPolicyStatus;
use tokio_codec::FramedRead;
use tokio_io::{io::WriteHalf, AsyncRead};

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

pub fn start_proxy(
    proxy_port: u16,
    policy: Addr<PolicyActor>,
) -> std::io::Result<Addr<TcpDataServer>> {
    let socket_in = SocketAddr::from(([0, 0, 0, 0], proxy_port));
    log::info!("starting TCP repeater on port {}", proxy_port);
    let listener = tokio_tcp::TcpListener::bind(&socket_in)?;
    // start server, listening for connections on a TCP socket
    let server = TcpDataServer::create(move |ctx| {
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(TcpConnect));
        TcpDataServer {
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
    policy: Addr<PolicyActor>,
    pub port: u16,
}

impl Actor for TcpDataServer {
    type Context = Context<Self>;
    fn stopped(&mut self, _ctx: &mut Context<Self>) {
        info!("stopped socket: {}", self.port);
    }
}

impl Handler<Stop> for TcpDataServer {
    type Result = ();
    fn handle(&mut self, _msg: Stop, ctx: &mut Context<Self>) -> Self::Result {
        ctx.stop()
    }
}

/// Notification of new TCP socket connection
struct TcpConnect(tokio_tcp::TcpStream);

impl Message for TcpConnect {
    type Result = Result<(), ()>;
}

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

fn shutdown_both(stream: tokio_tcp::TcpStream) {
    if let Err(e) = stream.shutdown(std::net::Shutdown::Both) {
        warn!("{}", e);
    }
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
                if socket.port() == self.port
                    && armour_data_interface::INTERFACE_IPS.contains(&socket.ip())
                {
                    warn!("TCP {}: trying to forward to self", self.port);
                    if let Err(e) = msg.0.shutdown(std::net::Shutdown::Both) {
                        warn!("{}", e);
                    };
                    Box::new(futures::future::ok(()))
                } else {
                    // For each incoming connection we create a `TcpData` actor
                    let policy = self.policy.clone();
                    let server = self
                        .policy
                        .send(tcp_policy::GetTcpPolicy(peer_addr, socket))
                        .then(move |allow| match allow {
                            // allow connection
                            Ok(Ok(TcpPolicyStatus::Allow(connection))) => future::Either::A(
                                tokio_tcp::TcpStream::connect(&socket).and_then(move |sock| {
                                    // create actor for handling connection
                                    TcpData::create(move |ctx| {
                                        msg.0.set_nodelay(true).unwrap();
                                        sock.set_nodelay(true).unwrap();
                                        msg.0
                                            .set_linger(Some(std::time::Duration::from_secs(60)))
                                            .unwrap();
                                        sock.set_linger(Some(std::time::Duration::from_secs(60)))
                                            .unwrap();
                                        let (r, wc) = msg.0.split();
                                        TcpData::add_stream(
                                            FramedRead::new(r, client::ClientCodec),
                                            ctx,
                                        );
                                        let (r, ws) = sock.split();
                                        TcpData::add_stream(
                                            FramedRead::new(r, server::ServerCodec),
                                            ctx,
                                        );
                                        TcpData {
                                            policy,
                                            tcp_client_framed: actix::io::FramedWrite::new(
                                                wc,
                                                client::ClientCodec,
                                                ctx,
                                            ),
                                            tcp_server_framed: actix::io::FramedWrite::new(
                                                ws,
                                                server::ServerCodec,
                                                ctx,
                                            ),
                                            connection: *connection,
                                        }
                                    });
                                    Ok(())
                                }),
                            ),
                            // reject
                            Ok(Ok(TcpPolicyStatus::Block)) => {
                                info!("connection denied");
                                shutdown_both(msg.0);
                                future::Either::B(future::ok(()))
                            }
                            // policy error
                            Ok(Err(e)) => {
                                warn!("{}", e);
                                shutdown_both(msg.0);
                                future::Either::B(future::ok(()))
                            }
                            // actor error
                            Err(e) => {
                                warn!("{}", e);
                                shutdown_both(msg.0);
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

/// Actor that handles TCP communication with a data plane instance
///
/// There will be one actor per TCP socket connection
struct TcpData {
    policy: Addr<PolicyActor>,
    tcp_client_framed: actix::io::FramedWrite<WriteHalf<tokio_tcp::TcpStream>, client::ClientCodec>,
    tcp_server_framed: actix::io::FramedWrite<WriteHalf<tokio_tcp::TcpStream>, server::ServerCodec>,
    connection: Option<tcp_policy::ConnectionStats>,
}

impl Actor for TcpData {
    type Context = Context<Self>;

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(connection) = &self.connection {
            self.policy.do_send(connection.clone());
        }
        info!("end of connection")
    }
}

// read from client becomes write to server
impl StreamHandler<client::ClientBytes, std::io::Error> for TcpData {
    fn handle(&mut self, msg: client::ClientBytes, _ctx: &mut Self::Context) {
        if let Some(connection) = self.connection.as_mut() {
            connection.sent += msg.0.len();
        }
        self.tcp_server_framed.write(msg.0)
    }
    fn finished(&mut self, ctx: &mut Context<Self>) {
        ctx.stop()
    }
}

// read from server becomes write to client
impl StreamHandler<server::ServerBytes, std::io::Error> for TcpData {
    fn handle(&mut self, msg: server::ServerBytes, _ctx: &mut Self::Context) {
        if let Some(connection) = self.connection.as_mut() {
            connection.received += msg.0.len();
        }
        self.tcp_client_framed.write(msg.0)
    }
}

impl actix::io::WriteHandler<std::io::Error> for TcpData {}
