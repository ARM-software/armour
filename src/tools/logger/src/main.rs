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

// use chrono_tz::GMT;
use logger::{connections::Connections, web, LoggerService};
use rustyline::{completion, error::ReadlineError, hint, Editor};
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};

fn main() -> std::io::Result<()> {
    // enable logging
    std::env::set_var("RUST_LOG", "logger=info,policy_service=info,actix=debug");
    std::env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    let args: Box<Vec<String>> = Box::new(std::env::args().collect());
    if args.len() == 2 {
        // start Actix system
        let mut sys = actix::System::new("logger");
        // start policy service actor
        let connections = Arc::new(Mutex::new(Connections::default()));
        let logger = LoggerService(connections.clone());
        let socket = Box::leak(args)[1].as_str();
        let policy_service = if socket.to_socket_addrs().is_ok() {
            sys.block_on(policy_service::start_tcp_policy_service(logger, socket))?
        } else {
            sys.block_on(policy_service::start_uds_policy_service(
                logger,
                std::path::PathBuf::from(socket),
            ))?
        };
        // start web server
        web::start_web_server(connections.clone(), 9000)?;
        // issue commands based on user input
        std::thread::spawn(move || {
            let mut rl = Editor::new();
            rl.set_helper(Some(Helper::new()));
            if rl.load_history("logger.txt").is_err() {
                log::info!("no previous history");
            }
            loop {
                match rl.readline("logger:> ") {
                    Ok(cmd) => {
                        let cmd = cmd.trim();
                        rl.add_history_entry(cmd);
                        match cmd {
                            "" => (),
                            "help" => {
                                log::info!("commands: help, clear, graph, show, summary, quit")
                            }
                            "clear" => {
                                let mut connections = connections.lock().unwrap();
                                connections.clear()
                            }
                            "graph" => {
                                let connections = connections.lock().unwrap();
                                connections
                                    .export_pdf("connections", true, false)
                                    .unwrap_or_else(|err| log::warn!("{}", err))
                            }
                            "show" => {
                                let connections = connections.lock().unwrap();
                                log::info!("{}", connections.to_yaml())
                            }
                            "summary" => {
                                let connections = connections.lock().unwrap();
                                log::info!("{}", connections.to_yaml_summary())
                            }
                            "quit" => {
                                // remove socket
                                policy_service.do_send(policy_service::Quit);
                                break;
                            }
                            _ => log::warn!("unknown command"),
                        }
                    }
                    Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                        // remove socket
                        policy_service.do_send(policy_service::Quit);
                        break;
                    }
                    Err(err) => log::warn!("{}", err),
                }
            }
            rl.save_history("logger.txt")
                .expect("failed to save history")
        });
        sys.run()
    } else {
        println!("usage: {} <socket>", args[0]);
        Ok(())
    }
}

struct Helper(completion::FilenameCompleter, hint::HistoryHinter);

impl Helper {
    fn new() -> Self {
        Helper(completion::FilenameCompleter::new(), hint::HistoryHinter {})
    }
}

impl completion::Completer for Helper {
    type Candidate = completion::Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> Result<(usize, Vec<completion::Pair>), ReadlineError> {
        self.0.complete(line, pos, ctx)
    }
}
impl hint::Hinter for Helper {
    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<String> {
        self.1.hint(line, pos, ctx)
    }
}

impl rustyline::highlight::Highlighter for Helper {}

impl rustyline::Helper for Helper {}
impl rustyline::validate::Validator for Helper {}
