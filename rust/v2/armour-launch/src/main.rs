use clap::{crate_version, App, Arg, SubCommand, AppSettings};
use std::collections::BTreeMap as Map;
use std::fs;
use std::process::Command;
use std::io::{Error, ErrorKind};
use awc::Client;
use std::io::{self, Write};
use tokio::prelude::Future;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let matches = App::new("armour-compose")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Basma El Gaabouri <basma.elgaabouri@arm.com>")
        .about("Armour launcher")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(SubCommand::with_name("up")
            .about("Start Armour compose")
            .version(crate_version!())
            .arg(Arg::with_name("input file")
                .index(1)
                .required(true)
                .help("Start armour compose")))
        .subcommand(SubCommand::with_name("down")
            .about("Stop Armour compose")
            .version(crate_version!())
            .arg(Arg::with_name("input file")
                .index(1)
                .help("Stop armour compose")))
        .get_matches();

    if let Some(up) = matches.subcommand_matches("up") {
        if let Some(filename) = up.value_of("input file") { //.unwrap_or("armour-compose.yml") {
            match armour_compose::Compose::from_path(filename) {
                Ok(compose) => {
                    let content = serde_yaml::to_string(&compose).unwrap();
                    let mut cmp: armour_compose::Compose = serde_yaml::from_str(&content).unwrap();
                    let mut info = Map::new();
                    let mut networks = Map::new();
                    let mut srv = Map::new();
                    let keys: Vec<_> = compose.services.keys().cloned().collect();
                    for service in keys {
                        if service.to_string().len() > 12 {
                            return Err(Error::new(ErrorKind::Other, "Service name too long, max 12 characters"));
                        }
                        
                        let mut driver_opt = Map::new();
                        let mut name: String = "arm-".to_owned();
                        name.push_str(&service.to_string());
                        let temp: armour_compose::service::Armour = compose.services.get(&service).unwrap().armour.clone();
                        info.insert(service.to_string(), armour_compose::service::MasterInfo { 
                            armour_labels: temp,
                            container_labels: compose.services.get(&service).unwrap().labels.clone(),
                            network: name.clone(),
                            ..Default::default()
                        });
                        driver_opt.insert("com.docker.network.bridge.name".to_string(),name.clone());
                        let net: armour_compose::network::Network = armour_compose::network::Network { 
                            driver: Some(armour_compose::network::Driver::Bridge),
                            driver_opts: driver_opt,
                            internal: true,
                            ..Default::default()
                        };
                        networks.insert(name.clone(),net);
                        
                        let def_arm = armour_compose::service::Armour { labels: armour_compose::serde_utils::array_dict::ArrayDict::Array(Vec::new())};
                        let def_net = armour_compose::network::Networks::Array([name.clone()].to_vec());
                        let def = armour_compose::service::Service { armour: def_arm, networks: def_net, ..cmp.services.get(&service).unwrap().clone()};
                        srv.insert(service.to_string(),def);
                        
                    }
                    cmp.services = srv;
                    cmp.networks = networks;
                    fs::write("docker-compose.yml", serde_yaml::to_string(&cmp).unwrap()).expect("Unable to write file");
                    // assuming the docker engine is running
                    //create containers
                    match Command::new("docker-compose")
                        .arg("up")
                        .arg("--no-start")
                        .output() {
                        Ok(child) => {
                            if child.status.success() {
                                println!("{:?}",io::stdout().write_all(&child.stdout).unwrap());
                            }
                            else {
                                panic!("{:?}",io::stderr().write_all(&child.stderr).unwrap());
                            }
                        }
                        Err(_err) => { panic!("failed to get a successful response status!"); }
                    }
                    //get networks and ips
                    let docker = shiplift::Docker::new();
                    for (key, _) in info.iter() {
                        let fut = docker.containers().get(key).inspect().map(|container| println!("{:#?}", container))
                        .map_err(|e| eprintln!("Error: {}", e));
                        tokio::run(fut);
                        // let ip = ...network_settings.ip_address;
                        
                    }
                    //should the created networks be sent with the labels ?
                    let client = Client::default();
                    let response = client.post("http://localhost:8088/on-boarding")
                    .send_json(&info)
                    .await
                    .map_err(|_| ())
                        .and_then(|_response| { 
                            Ok(())
                        });
                    match response {
                        Ok(_) => {
                            match Command::new("docker-compose")
                                .arg("up")
                                .output() {
                                Ok(child) => {
                                    if child.status.success() {
                                        println!("{:?}",io::stdout().write_all(&child.stdout).unwrap());
                                    }
                                    else {
                                        panic!("{:?}",io::stderr().write_all(&child.stderr).unwrap());
                                    }
                                }
                                Err(_err) => { panic!("failed to get a successful response status!"); }
                            }
                        },
                        Err(_error) => { panic!("failed to get a successful response status!"); }
                    };
                    println!("end");

                }
                Err(e) => println!("{}", e),
            }
        } 
    } else if let Some(down) = matches.subcommand_matches("down") {
        if let Some(filename) = down.value_of("input file") { //.unwrap_or("docker-compose.yml") {
            match armour_compose::Compose::from_path(filename) {
                Ok(_compose) => {
                    match Command::new("docker-compose")
                        .arg("down")
                        .output() {
                        Ok(child) => {
                            if child.status.success() {
                                println!("{:?}",io::stdout().write_all(&child.stdout).unwrap());
                            }
                            else {
                                panic!("{:?}",io::stderr().write_all(&child.stderr).unwrap());
                            }
                        }
                        Err(_err) => { panic!("failed to get a successful response status!"); }
                    }
                }
                Err(e) => println!("{}", e),
            }
        }
    }
    Ok(())
}