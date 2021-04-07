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

use actix::{Actor, System};
use actix::prelude::*;
use tokio_uds;
use tokio_io::{AsyncRead};

#[macro_use]
extern crate log;
extern crate env_logger;

use iptables_lib as lib;

fn main() {

    env_logger::init();

    let system = System::new("iptables-client");

    let ars : Vec<String> = std::env::args().collect();
    let socket = ars[1].clone();

    info!("Address is {}", socket.to_string());

    Arbiter::spawn(
        tokio_uds::UnixStream::connect(socket.to_string())
            .and_then(|stream| {
                let addr = lib::IptablesClientActor::create(|ctx| {
                    let (_r, w) = stream.split();
                    lib::IptablesClientActor {
                        uds_framed: actix::io::FramedWrite::new(
                            w,
                            lib::IptablesCodec,
                            ctx,
                        ),
                    }
                });
                addr.do_send(
                    lib::IptablesCommands::NewChain{
                        table: "nat".to_string(),
                        chain: "mychain".to_string()}
                );
                addr.do_send(
                    lib::IptablesCommands::DeleteChain {
                        table: "nat".to_string(),
                        chain: "mychain".to_string()}
                );
                futures::future::ok(())
            })
            .map_err(|_| ())
    );

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
