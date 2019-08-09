use std::io::prelude::*;
use std::net::TcpListener;

fn main() {
    TcpListener::bind("127.0.0.1:8443")
        .expect("failed to bind")
        .incoming()
        .for_each(|sock| {
            let mut sock = sock.unwrap();
            let mut buf = [0; 32];
            sock.read(&mut buf).unwrap();
            println!("received: {:?}", String::from_utf8(buf.to_vec()).unwrap());
            sock.write(b"heard you").unwrap();
        })
}