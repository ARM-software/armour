use crate::oracle_capnp::oracle;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{Future, Stream};
use tokio::io::AsyncRead;
use tokio::runtime::current_thread;

struct OracleImpl;

impl oracle::Server for OracleImpl {
    fn eval(
        &mut self,
        calls: oracle::EvalParams,
        mut results: oracle::EvalResults,
    ) -> Promise<(), capnp::Error> {
        let calls = pry!(pry!(calls.get()).get_calls());
        for call in calls.iter() {
            println!("Method: {}", pry!(call.get_method()));
            for arg in pry!(call.get_args()).iter() {
                match pry!(arg.which()) {
                    oracle::value::Which::Bool(b) => println!("Arg: Bool({})", b),
                    oracle::value::Which::Int64(i) => println!("Arg: Int64({})", i),
                    oracle::value::Which::Float64(f) => println!("Arg: Float64({})", f),
                    oracle::value::Which::Text(t) => println!("Arg: Text({})", pry!(t)),
                    oracle::value::Which::Data(d) => println!("Arg: Data({:?})", pry!(d)),
                }
            }
        }
        let mut rs = results.get().init_results(2);
        rs.reborrow().get(0).set_float64(3.141);
        rs.get(1).set_text("that worked!");

        Promise::ok(())
    }
    fn update(
        &mut self,
        params: oracle::UpdateParams,
        _: oracle::UpdateResults,
    ) -> Promise<(), capnp::Error> {
        let _x = pry!(params.get()).get_calls();
        Promise::ok(())
    }
}

pub fn main() -> Result<(), capnp::Error> {
    use std::net::ToSocketAddrs;
    let args: Vec<String> = ::std::env::args().collect();
    if args.len() != 3 {
        println!("usage: {} server ADDRESS[:PORT]", args[0]);
        return Ok(());
    }

    let addr = args[2]
        .to_socket_addrs()
        .unwrap()
        .next()
        .expect("could not parse address");
    let socket = tokio::net::TcpListener::bind(&addr).unwrap();

    let oracle = oracle::ToClient::new(OracleImpl).into_client::<::capnp_rpc::Server>();

    let done = socket.incoming().for_each(move |socket| {
        socket.set_nodelay(true)?;
        let (reader, writer) = socket.split();

        let network = twoparty::VatNetwork::new(
            reader,
            writer,
            rpc_twoparty_capnp::Side::Server,
            Default::default(),
        );

        let rpc_system = RpcSystem::new(Box::new(network), Some(oracle.clone().client));
        current_thread::spawn(rpc_system.map_err(|e| println!("error: {:?}", e)));
        Ok(())
    });

    current_thread::block_on_all(done).unwrap();
    Ok(())
}
