use crate::oracle_capnp::oracle;
use capnp::capability::Promise;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::Future;
use tokio::io::AsyncRead;

pub fn main() -> Result<(), capnp::Error> {
    use std::net::ToSocketAddrs;
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        println!("usage: {} client ADDRESS[:PORT]", args[0]);
        return Ok(());
    }

    let mut runtime = tokio::runtime::current_thread::Runtime::new().unwrap();

    let addr = args[2]
        .to_socket_addrs()
        .unwrap()
        .next()
        .expect("could not parse address");
    let stream = runtime
        .block_on(tokio::net::TcpStream::connect(&addr))
        .unwrap();
    stream.set_nodelay(true).expect("no delay");
    let (reader, writer) = stream.split();

    let network = Box::new(twoparty::VatNetwork::new(
        reader,
        writer,
        rpc_twoparty_capnp::Side::Client,
        Default::default(),
    ));
    let mut rpc_system = RpcSystem::new(network, None);
    let oracle: oracle::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
    runtime.spawn(rpc_system.map_err(|_e| ()));

    println!("Eval request...");
    let mut request = oracle.eval_request();
    let mut calls = request.get().init_calls(2);
    {
        let mut call = calls.reborrow().get(0);
        call.set_method("test1");
        let mut args = call.init_args(3);
        args.reborrow().get(0).set_bool(false);
        args.reborrow().get(1).set_int64(3);
        args.get(2).set_data(b"some data");
    }
    {
        let mut call = calls.get(1);
        call.set_method("test2");
        let mut args = call.init_args(2);
        args.reborrow().get(0).set_float64(3.141);
        args.get(1).set_text("hello");
    }
    runtime.block_on(
        request
            .send()
            .promise
            .and_then(|response| {
                let results = pry!(pry!(response.get()).get_results());
                if results.len() == 0 {
                    println!("Got no results")
                } else {
                    for result in results.iter() {
                        match pry!(result.which()) {
                            oracle::value::Which::Bool(b) => println!("Res: Bool({})", b),
                            oracle::value::Which::Int64(i) => println!("Res: Int64({})", i),
                            oracle::value::Which::Float64(f) => println!("Res: Float64({})", f),
                            oracle::value::Which::Text(t) => println!("Res: Text({})", pry!(t)),
                            oracle::value::Which::Data(d) => println!("Res: Data({:?})", pry!(d)),
                        }
                    }
                }
                Promise::ok(())
            })
            .or_else(|err| {
                println!("Got error: {}", err);
                Promise::err(err)
            }),
    )?;
    println!("PASS");
    Ok(())
}
