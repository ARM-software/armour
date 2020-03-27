use armour_api::master::OnboardInformation;
use armour_compose::Compose;
use awc::Client;
use clap::{crate_version, App, AppSettings, Arg, SubCommand};
use std::process::Command;

#[actix_rt::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let matches = get_matches();
    if let Some(up) = matches.subcommand_matches("up") {
        // read armour-compose from input file and write docker-compose.yml file
        let mut info = read_armour(up.value_of("input file").unwrap())?;
        // try to run `docker-compose up` command
        docker_up()?;
        // try to set IP addresses for containers (leaves containers in paused state)
        set_ip_addresses(&mut info).await;
        // notify data plane master - onboarding
        const TCP_PORT: u16 = 8090;
        let port = matches
            .value_of("port")
            .map(|s| s.parse::<u16>().unwrap_or(TCP_PORT))
            .unwrap_or(TCP_PORT);
        let client = Client::default();
        match client
            .post(format!("http://localhost:{}/on-board", port))
            .send_json(&info)
            .await
            .map(|res| res.status().is_success())
        {
            Ok(true) => {
                println!("onboarding succeeded");
                unpause_all(&info).await;
                Ok(())
            }
            Ok(false) => {
                docker_down()?;
                Err("onboarding failed".into())
            }
            Err(e) => {
                docker_down()?;
                Err(format!("onboarding failed: {}", e).into())
            }
        }
    } else if let Some(down) = matches.subcommand_matches("down") {
        // create docker-compose.yml from armour-compose input file
        read_armour(down.value_of("input file").unwrap())?;
        // try to run `docker-compose down` command
        docker_down()
    // TODO: notify master
    } else {
        unreachable!()
    }
}

fn docker_up() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let child = Command::new("docker-compose")
        .arg("up")
        .arg("--no-start")
        .output()?;
    if child.status.success() {
        println!("`docker-compose up` successfull");
        if !child.stdout.is_empty() {
            println!("{}", String::from_utf8(child.stdout)?)
        }
        Ok(())
    } else {
        Err(String::from_utf8(child.stderr)?.into())
    }
}

fn docker_down() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let child = Command::new("docker-compose").arg("down").output()?;
    if child.status.success() {
        println!("`docker-compose down` successful");
        if !child.stdout.is_empty() {
            println!("{}", String::from_utf8(child.stdout)?)
        }
        Ok(())
    } else {
        Err(String::from_utf8(child.stderr)?.into())
    }
}

fn read_armour<P: AsRef<std::path::Path>>(
    p: P,
) -> Result<OnboardInformation, Box<dyn std::error::Error + Send + Sync>> {
    // load armour compose file
    let (compose, info) = Compose::read_armour(p)?;
    // save as docker compose file
    std::fs::write("docker-compose.yml", serde_yaml::to_string(&compose)?)?;
    Ok(info)
}

// async fn stop_all(information: &OnboardInformation) {
//     // try to get IP addresses for containers
//     let docker = docker_api::Docker::new();
//     for name in information.keys() {
//         if let Err(e) = docker.stop_container(name).await {
//             println!("warn: {}", e)
//         } else if let Err(e) = docker.remove_container(name).await {
//             println!("warn: {}", e)
//         } else {
//             println!("stopped: {}", name)
//         }
//     }
// }

async fn unpause_all(information: &OnboardInformation) {
    // try to get IP addresses for containers
    let docker = docker_api::Docker::new();
    for name in information.keys() {
        if let Err(e) = docker.unpause_container(name).await {
            println!("warn: {}", e)
        } else {
            println!("unpaused: {}", name)
        }
    }
}

async fn set_ip_addresses(information: &mut OnboardInformation) {
    // try to get IP addresses for containers
    let docker = docker_api::Docker::new();
    for (name, info) in information.iter_mut() {
        if let Err(e) = docker.start_container(name).await {
            println!("warn: {}", e)
        } else if let Err(e) = docker.pause_container(name).await {
            println!("warn: {}", e)
        } else {
            match docker.inspect_container(&name).await {
                Ok(container) => info.ipv4_address = get_ip_address(container),
                Err(e) => println!("warn: {}", e),
            }
        }
    }
}

fn get_ip_address(container: docker_api::rep::ContainerDetails) -> Option<std::net::Ipv4Addr> {
    container
        .network_settings
        .networks
        .iter()
        .next()
        .map(|(_, network)| network.ip_address.parse().ok())
        .flatten()
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
            Arg::with_name("port")
                .short("p")
                .required(false)
                .takes_value(true)
                .help("TCP port for data plane master"),
        )
        .subcommand(
            SubCommand::with_name("up")
                .about("Start Armour compose")
                .version(crate_version!())
                .arg(
                    Arg::with_name("input file")
                        .index(1)
                        .required(true)
                        .help("Start armour compose"),
                ),
        )
        .subcommand(
            SubCommand::with_name("down")
                .about("Stop Armour compose")
                .version(crate_version!())
                .arg(
                    Arg::with_name("input file")
                        .index(1)
                        .required(true)
                        .help("Stop armour compose"),
                ),
        )
        .get_matches()
}
