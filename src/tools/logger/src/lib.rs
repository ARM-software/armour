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

use policy_service::rpc::Literal;
use std::sync::{Arc, Mutex};

pub mod connections;
mod dotgraph;
mod graph;
pub mod web;

pub struct LoggerService(pub Arc<Mutex<connections::Connections>>);

impl policy_service::rpc::Dispatcher for LoggerService {
    fn dispatch(&mut self, name: &str, args: &[Literal]) -> Result<Literal, capnp::Error> {
        match (name, args) {
            ("ping", _) => Ok(Literal::Unit),
            ("log", _) => {
                self.log(name, args);
                Ok(Literal::Unit)
            }
            (
                "rest",
                [Literal::Int(number), Literal::Str(date), Literal::Str(method), Literal::Str(path), Literal::Tuple(from), Literal::Tuple(to), Literal::Int(size)],
            ) if from.len() == 4 && to.len() == 4 => {
                if let (Some(from), Some(to)) = (
                    connections::Endpoint::from(from),
                    connections::Endpoint::to(to),
                ) {
                    let connection = connections::Connection::new(
                        connections::Info::rest(date, method, path, *size as usize),
                        from,
                        to,
                    );
                    log::debug!("[{}]: {:?}", number, connection);
                    let mut connections = self.0.lock().unwrap();
                    connections.add_connection(*number as u64, connection)
                } else {
                    log::warn!("incomplete ID");
                    self.log(name, args)
                }
                log::debug!("logged REST connection");
                Ok(Literal::Unit)
            }
            ("server_payload", [Literal::Int(number), Literal::Int(size)]) => {
                log::debug!("[{}]: server payload: {}", number, size);
                let mut connections = self.0.lock().unwrap();
                connections.set_received(*number as u64, *size as usize);
                Ok(Literal::Unit)
            }
            ("tcp", [Literal::Int(number), Literal::Tuple(from), Literal::Tuple(to)])
                if from.len() == 3 && to.len() == 3 =>
            {
                if let (Some(from), Some(to)) = (
                    connections::Endpoint::from(from),
                    connections::Endpoint::to(to),
                ) {
                    let connection =
                        connections::Connection::new(connections::Info::tcp(), from, to);
                    log::debug!("{:?}", connection);
                    let mut connections = self.0.lock().unwrap();
                    connections.add_connection(*number as u64, connection)
                } else {
                    log::warn!("incomplete ID");
                    self.log(name, args)
                }
                log::debug!("logged TCP connection");
                Ok(Literal::Unit)
            }
            ("tcp_stats", [Literal::Int(number), Literal::Int(sent), Literal::Int(received)]) => {
                log::debug!("[{}]: sent: {}; received: {}", number, sent, received);
                let mut connections = self.0.lock().unwrap();
                let n = *number as u64;
                connections.set_sent(n, *sent as usize);
                connections.set_received(n, *received as usize);
                Ok(Literal::Unit)
            }
            _ => {
                self.log(name, args);
                Err(capnp::Error::failed("bad call".to_string()))
            }
        }
    }
}
