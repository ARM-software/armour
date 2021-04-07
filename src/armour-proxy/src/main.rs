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

use armour_proxy::{http_proxy, policy::PolicyActor};
use clap::{crate_version, App as ClapApp, Arg};
use std::convert::TryInto;
use std::env;

fn main() -> std::io::Result<()> {
    // CLI
    let matches = ClapApp::new("armour-proxy")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Gustavo Petri <gustavo.petri@arm.com>")
        .about("Armour Proxy, with support for Security Policies")
        .arg(
            Arg::with_name("host socket")
                .index(1)
                .required(true)
                .help("Unix socket of data plane host"),
        )
        .arg(
            Arg::with_name("label")
                .long("label")
                .takes_value(true)
                .required(false)
                .help("label for proxy instance"),
        )
        .arg(
            Arg::with_name("timeout")
                .long("timeout")
                .takes_value(true)
                .required(false)
                .help("HTTP timeout"),
        )
        .arg(
            Arg::with_name("log level")
                .short("l")
                .takes_value(true)
                .possible_values(&["error", "warn", "info", "debug", "trace"])
                .help("log level"),
        )
        .get_matches();

    let log_level = matches.value_of("log level").unwrap_or("debug");

    // enable logging
    env::set_var(
        "RUST_LOG",
        format!(
            "armour_proxy={l},armour_lang={l},actix_web={l}",
            l = log_level
        ),
    );
    env::set_var("RUST_BACKTRACE", "0");
    pretty_env_logger::init();

    // get Armour key
    let key: [u8; 32] = base64::decode(
        env::var("ARMOUR_PASS")
            .expect("ARMOUR_PASS environment variable not set")
            .as_str(),
    )
    .expect("ARMOUR_PASS is not base64 encoded")
    .as_slice()
    .try_into()
    .expect("ARMOUR_PASS is wrong length");
    // let key = [0; 32];

    // start Actix system
    let mut sys = actix_rt::System::new("armour_proxy");

    log::info!("local host names are: {:?}", *http_proxy::LOCAL_HOST_NAMES);

    // start up policy actor
    // (should possibly use actix::sync::SyncArbiter)
    // install the CLI policy
    let host_socket = matches.value_of("host socket").unwrap().to_string();
    log::info!("connecting to: {}", host_socket);
    let stream = sys.block_on(tokio::net::UnixStream::connect(host_socket))?;
    let timeout = matches
        .value_of("timeout")
        .map(|s| s.parse::<u8>().ok())
        .flatten()
        .unwrap_or(5);
    match matches.value_of("label").unwrap_or("proxy").parse() {
        Ok(label) => {
            PolicyActor::create_policy(stream, label, timeout, key);
            sys.run()
        }
        Err(err) => {
            log::warn!("{}", err);
            Ok(())
        }
    }
}
