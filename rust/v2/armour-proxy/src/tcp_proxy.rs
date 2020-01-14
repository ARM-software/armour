// The following is needed for testing:

// Terminal 1 (for armour-master)
// - docker-machine create --virtualbox-cpu-count "2" --virtualbox-memory "4096" armour
// - docker-machine ssh armour
// - sudo iptables -t nat -I PREROUTING -i armour -p tcp --dport 443 -j DNAT --to-destination 127.0.0.1:8443
// - sudo iptables -t nat -I PREROUTING -i armour -p tcp --dport 5001 -j DNAT --to-destination 127.0.0.1:6000
// - sudo sysctl -w net.ipv4.conf.armour.route_localnet=1
// - $TARGET_PATH/armour-master

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
use futures::StreamExt;
use policy::PolicyActor;
use std::net::SocketAddr;
use tcp_policy::TcpPolicyStatus;
use tokio::io::WriteHalf;
use tokio_util::codec::FramedRead;

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

pub async fn start_proxy(
    proxy_port: u16,
    policy: Addr<PolicyActor>,
) -> std::io::Result<Addr<TcpDataServer>> {
    let socket_in = SocketAddr::from(([0, 0, 0, 0], proxy_port));
    log::info!("starting TCP repeater on port {}", proxy_port);
    let listener = Box::new(tokio::net::TcpListener::bind(&socket_in).await?);
    // start server, listening for connections on a TCP socket
    let server = TcpDataServer::create(move |ctx| {
        ctx.add_stream(
            Box::leak(listener)
                .incoming()
                .map(|st| TcpConnect(st.unwrap())),
        );
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

// obtain the original socket destination (SO_ORIGINAL_DST)
// we assume Linux's `iptables` have been used to redirect connections to the proxy
#[cfg(target_os = "linux")]
fn original_dst(sock: &tokio::net::TcpStream) -> Option<std::net::SocketAddr> {
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
fn original_dst(_sock: &tokio::net::TcpStream) -> Option<std::net::SocketAddr> {
    None
}

fn shutdown_both(stream: tokio::net::TcpStream) {
    if let Err(e) = stream.shutdown(std::net::Shutdown::Both) {
        warn!("{}", e);
    }
}

// const LINGER_TIME: u64 = 60;

/// Notification of new TCP socket connection
#[derive(Message)]
#[rtype("()")]
struct TcpConnect(tokio::net::TcpStream);

impl StreamHandler<TcpConnect> for TcpDataServer {
    // type Result = ();
    fn finished(&mut self, _ctx: &mut Self::Context) {
        log::info!("TCP connect finished");
    }
    fn handle(&mut self, msg: TcpConnect, ctx: &mut Context<Self>) {
        log::info!("TcpConnect");
        // get the orgininal server socket address
        if let Some(socket) = original_dst(&msg.0) {
            if let Ok(peer_addr) = msg.0.peer_addr() {
                info!(
                    "TCP {}: received from {}, forwarding to {}",
                    self.port, peer_addr, socket
                );
                if socket.port() == self.port && armour_api::INTERFACE_IPS.contains(&socket.ip()) {
                    warn!("TCP {}: trying to forward to self", self.port);
                    if let Err(e) = msg.0.shutdown(std::net::Shutdown::Both) {
                        warn!("{}", e);
                    };
                } else {
                    let policy = self.policy.clone();
                    self.policy
                        .send(tcp_policy::GetTcpPolicy(peer_addr, socket))
                        .into_actor(self)
                        .then(move |allow, act, _ctx| {
                            async move {
                                match allow {
                                    Ok(Ok(TcpPolicyStatus::Allow(connection))) => {
                                        // For each incoming connection we create a `TcpData` actor
                                        if let Ok(sock) =
                                            tokio::net::TcpStream::connect(&socket).await
                                        {
                                            TcpData::create(move |ctx| {
                                                let (r, wc) = tokio::io::split(msg.0);
                                                ctx.add_stream(FramedRead::new(
                                                    r,
                                                    client::ClientCodec,
                                                ));
                                                let (r, ws) = tokio::io::split(sock);
                                                ctx.add_stream(FramedRead::new(
                                                    r,
                                                    server::ServerCodec,
                                                ));
                                                TcpData {
                                                    policy,
                                                    counter: 0,
                                                    client_writer: actix::io::Writer::new(wc, ctx),
                                                    server_writer: actix::io::Writer::new(ws, ctx),
                                                    connection: *connection,
                                                }
                                            });
                                        } else {
                                            warn!("failed to connect to socket: {}", socket);
                                            shutdown_both(msg.0)
                                        }
                                    }
                                    // reject
                                    Ok(Ok(TcpPolicyStatus::Block)) => {
                                        info!("connection denied");
                                        shutdown_both(msg.0)
                                    }
                                    // policy error
                                    Ok(Err(e)) => {
                                        warn!("{}", e);
                                        shutdown_both(msg.0)
                                    }
                                    // actor error
                                    Err(e) => {
                                        warn!("{}", e);
                                        shutdown_both(msg.0)
                                    }
                                }
                            }
                            .into_actor(act)
                        })
                        .wait(ctx)
                }
            } else {
                warn!("TCP {}: could not obtain source IP address", self.port)
            }
        } else {
            warn!("TCP {}: could not obtain original destination", self.port)
        }
    }
}

/// Actor that handles TCP communication with a data plane instance
///
/// There will be one actor per TCP socket connection
pub struct TcpData {
    policy: Addr<PolicyActor>,
    counter: usize,
    client_writer: actix::io::Writer<WriteHalf<tokio::net::TcpStream>, std::io::Error>,
    server_writer: actix::io::Writer<WriteHalf<tokio::net::TcpStream>, std::io::Error>,
    connection: Option<tcp_policy::ConnectionStats>,
}

impl Actor for TcpData {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(connection) = &self.connection {
            self.policy.do_send(connection.clone());
        }
        info!("end of connection")
    }
}

impl TcpData {
    fn hb(&self, ctx: &mut actix::Context<Self>) {
        ctx.run_later(std::time::Duration::from_millis(500), |act, ctx| {
            // println!("{}", act.counter);
            act.counter = 0;
            act.hb(ctx);
        });
    }
}

const MAX_SPEED: usize = 100_000;

// read from client becomes write to server
impl StreamHandler<Result<client::ClientBytes, std::io::Error>> for TcpData {
    fn handle(
        &mut self,
        msg: Result<client::ClientBytes, std::io::Error>,
        ctx: &mut Self::Context,
    ) {
        // check if we are being flooded with client bytes and
        // if we are then simply close the connection
        // TODO: find a better way to handle backpressure
        if self.counter > MAX_SPEED {
            warn!("too fast, giving up!");
            ctx.stop();
        } else if let Ok(client::ClientBytes(bytes)) = msg {
            self.counter += 1;
            if let Some(connection) = self.connection.as_mut() {
                connection.sent += bytes.len();
            }
            self.server_writer.write(&bytes)
        }
    }
    fn finished(&mut self, ctx: &mut Context<Self>) {
        ctx.stop()
    }
}

// read from server becomes write to client
impl StreamHandler<Result<server::ServerBytes, std::io::Error>> for TcpData {
    fn handle(
        &mut self,
        msg: Result<server::ServerBytes, std::io::Error>,
        ctx: &mut Self::Context,
    ) {
        if self.counter > MAX_SPEED {
            warn!("too fast, giving up!");
            ctx.stop()
        } else if let Ok(server::ServerBytes(bytes)) = msg {
            self.counter += 1;
            if let Some(connection) = self.connection.as_mut() {
                connection.received += bytes.len();
            }
            self.client_writer.write(&bytes)
        }
    }
}

impl actix::io::WriteHandler<std::io::Error> for TcpData {}
