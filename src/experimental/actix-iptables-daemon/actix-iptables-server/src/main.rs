/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use actix::*;
use futures::{Stream, Future};
use actix::{Actor, Context, System, Handler};
use actix::AsyncContext;
use tokio_uds;
use tokio_codec::{FramedRead};
use tokio_io::{AsyncRead};

#[macro_use]
extern crate log;
extern crate env_logger;

use iptables_lib as lib;

#[derive(Default)]
pub struct IptablesActorServer {
    pub socket: String, 
}

impl Actor for IptablesActorServer {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Context<Self>) {
        info!("Iptables server started");
    }

    fn stopped(&mut self, _: &mut Context<Self>) {
        info!("Iptables server terminated");
    }

}

impl Handler<lib::UdsConnect> for IptablesActorServer {
    type Result = ();
    
    fn handle(&mut self, msg: lib::UdsConnect, _: &mut Context<Self>) {
        lib::IptablesActor::create(move |ctx| {
            let (r, w) = msg.0.split();
            lib::IptablesActor::add_stream(FramedRead::new(r, lib::IptablesCodec), ctx);
            lib::IptablesActor {
                uds_framed: actix::io::FramedWrite::new(w, lib::IptablesCodec, ctx),
                ipt: iptables::new(false).unwrap(),
            } 
        });
    }
}

fn main() {
    env_logger::init();

    let system = System::new("iptables-daemon");

    let ars : Vec<String> = std::env::args().collect();
    let socket = ars[1].clone();

    info!("Address is {}", socket.to_string());
    
    let listener = tokio_uds::UnixListener::bind(socket.to_string()).unwrap();
    
    let _server = IptablesActorServer::create(move |ctx| {
        ctx.add_message_stream(
            listener
                .incoming()
                .map_err(|_| ())
                .map(|st| lib::UdsConnect(st)),
        );
        IptablesActorServer{
            socket: socket,
        }
    });

    let ctrl_c = tokio_signal::ctrl_c().flatten_stream();
    let handle_shutdown = ctrl_c
        .for_each(|()| {
            println!("Ctrl-C received, shutting down");
            System::current().stop();
            Ok(())
        })
        .map_err(|_| ());
    actix::spawn(handle_shutdown);
    
    let _ = system.run();
}
