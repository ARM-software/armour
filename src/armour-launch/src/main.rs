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

use armour_compose::OnboardInfo;
use armour_launch::{
    already_running, docker_down, docker_up, drop_services, onboard_services, read_armour, rules,
    set_ip_addresses,
};
use armour_utils::parse_https_url;
use clap::{crate_version, App, AppSettings, Arg, SubCommand};
use std::collections::BTreeMap as Map;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    // enable logging
    std::env::set_var("RUST_LOG", "armour_utils=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    let matches = get_matches();
    let host_url = parse_https_url(
        matches
            .value_of("host")
            .unwrap_or(armour_api::host::DATA_PLANE_HOST),
        8090,
    )?;

    const FILE: &str = "docker-compose.yml";
    let out_file = matches.value_of("file").unwrap_or(FILE);
    let in_file = matches.value_of("input file").unwrap();
    let client = || {
        let ca = matches
            .value_of("ca")
            .unwrap_or("certificates/armour-ca.pem");
        let certificate_password = matches.value_of("certificate password").unwrap_or("armour");
        let certificate = matches
            .value_of("certificate")
            .unwrap_or("certificates/armour-launch.p12");
        armour_utils::client(&ca, &certificate_password, &certificate)
    };
    if let Some(_up) = matches.subcommand_matches("up") {
        // read armour-compose from input file and write docker-compose.yml file
        let infos = read_armour(in_file, out_file)?;
        for mut info in infos.into_iter() {
            // check if application is already running
            if already_running(&info).await {
                return Err("already running! run armour-lauch `down` first?".into());
            } else {
                // try to run `docker-compose up` command
                docker_up(out_file)?;
                // try to set IP addresses for containers (leaves containers in paused state)
                set_ip_addresses(&mut info).await;
                // notify data plane host - onboarding
                onboard_services(client()?, host_url.clone(), info, out_file).await?;
            }
        }
        Ok(())
    } else if let Some(_down) = matches.subcommand_matches("down") {
        // create docker-compose.yml from armour-compose input file
        let infos = read_armour(in_file, out_file)?;
        // try to run `docker-compose down` command
        docker_down(out_file)?;
        for info in infos.iter() {
            drop_services(client()?, host_url.clone(), info.proxies.clone()).await?;
        }
        Ok(())
    } else if let Some(rules_matches) = matches.subcommand_matches("rules") {
        let (compose, infos) = armour_compose::Compose::read_armour(in_file)?;
        let rules_file = rules_matches.value_of("rules file").unwrap_or("rules");
        let info = OnboardInfo {
            proxies: infos.iter().fold(Vec::new(), |acc, e| [&acc, e.proxies.as_slice()].concat()),
            services: Map::default(),//Not use in rules
        };
        rules(compose, infos, (&info).into(), std::ffi::OsStr::new(rules_file))
        //rules(compose, (&info).iter().map(|i| i.into()).collect(), std::ffi::OsStr::new(rules_file))
    } else {
        unreachable!()
    }
}

fn get_matches<'a>() -> clap::ArgMatches<'a> {
    App::new("armour-compose")
        .version(crate_version!())
        .author(
            "Anthony Fox <anthony.fox@arm.com> and Basma El Gaabouri <basma.elgaabouri@arm.com>",
        )
        .about("Armour launcher")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("ca")
                .long("ca")
                .required(false)
                .takes_value(true)
                .value_name("PEM file")
                .help("Certificate Authority for HTTPS"),
        )
        .arg(
            Arg::with_name("certificate password")
                .long("pass")
                .required(false)
                .takes_value(true)
                .help("Password for certificate"),
        )
        .arg(
            Arg::with_name("certificate")
                .long("cert")
                .required(false)
                .takes_value(true)
                .value_name("pkcs12 file")
                .help("Certificate for mTLS"),
        )
        .arg(
            Arg::with_name("host")
                .short("m")
                .long("host")
                .required(false)
                .takes_value(true)
                .value_name("URL")
                .help("data plane host URL"),
        )
        .arg(
            Arg::with_name("file")
                .short("f")
                .required(false)
                .takes_value(true)
                .help("alternate compose file"),
        )
        .arg(Arg::with_name("input file").required(true))
        .subcommand(SubCommand::with_name("up").about("Start Armour compose"))
        .subcommand(SubCommand::with_name("down").about("Stop Armour compose"))
        .subcommand(
            SubCommand::with_name("rules")
                .about("Generate iptables rules")
                .arg(Arg::with_name("rules file").required(false)),
        )
        .get_matches()
}
