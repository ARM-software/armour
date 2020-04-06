use armour_api::master::OnboardInformation;
use armour_compose::Compose;
use awc::Client;
use clap::{crate_version, App, AppSettings, Arg, SubCommand};
use std::process::Command;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    let matches = get_matches();
    const TCP_PORT: u16 = 8090;
    let master_port = matches
        .value_of("port")
        .map(|s| s.parse::<u16>().unwrap_or(TCP_PORT))
        .unwrap_or(TCP_PORT);
    if let Some(up) = matches.subcommand_matches("up") {
        // read armour-compose from input file and write docker-compose.yml file
        let mut info = read_armour(up.value_of("input file").unwrap())?;
        // check if application is already running
        if already_running(&info).await {
            return Err("already running! run armour-lauch `down` first?".into());
        } else {
            // try to run `docker-compose up` command
            docker_up()?;
            // try to set IP addresses for containers (leaves containers in paused state)
            set_ip_addresses(&mut info).await;
            // notify data plane master - onboarding
            onboard_services(master_port, &info).await
        }
    } else if let Some(down) = matches.subcommand_matches("down") {
        // create docker-compose.yml from armour-compose input file
        let info = read_armour(down.value_of("input file").unwrap())?;
        // try to run `docker-compose down` command
        docker_down()?;
        drop_services(master_port, &info).await
    } else {
        unreachable!()
    }
}

async fn onboard_services(master_port: u16, info: &OnboardInformation) -> Result<(), Error> {
    let client = Client::default();
    match client
        .post(format!(
            "http://localhost:{}/launch/on-board-services",
            master_port
        ))
        .send_json(info)
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                println!("onboarding succeeded");
                unpause_all(info).await;
                Ok(())
            } else {
                docker_down()?;
                drop_services(master_port, info).await?;
                Err(message(res)
                    .await
                    .unwrap_or_else(|| "onboarding failed".to_string())
                    .into())
            }
        }
        Err(e) => {
            docker_down()?;
            drop_services(master_port, info).await?;
            Err(format!("onboarding failed: {}", e).into())
        }
    }
}

async fn drop_services(master_port: u16, info: &OnboardInformation) -> Result<(), Error> {
    let client = Client::default();
    let res = client
        .delete(format!(
            "http://localhost:{}/launch/drop-services",
            master_port
        ))
        .send_json(info)
        .await
        .map_err(|err| format!("drop services failed: {}", err))?;
    if res.status().is_success() {
        println!("drop services succeeded");
        Ok(())
    } else {
        Err(message(res)
            .await
            .unwrap_or_else(|| "drop services failed".to_string())
            .into())
    }
}

async fn message(
    mut res: awc::ClientResponse<
        impl futures::stream::Stream<Item = Result<bytes::Bytes, awc::error::PayloadError>> + Unpin,
    >,
) -> Option<String> {
    let body = res.body().await.ok()?;
    let message = std::str::from_utf8(&body).ok()?;
    if message.is_empty() {
        None
    } else {
        Some(message.to_string())
    }
}

fn docker_up() -> Result<(), Error> {
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

fn docker_down() -> Result<(), Error> {
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

fn read_armour<P: AsRef<std::path::Path>>(p: P) -> Result<OnboardInformation, Error> {
    // load armour compose file
    let (compose, info) = Compose::read_armour(p)?;
    // save as docker compose file
    std::fs::write("docker-compose.yml", serde_yaml::to_string(&compose)?)?;
    Ok(info)
}

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

async fn already_running(information: &OnboardInformation) -> bool {
    // try to get IP addresses for containers
    let docker = docker_api::Docker::new();
    for name in information.keys() {
        if docker
            .inspect_container(&name)
            .await
            .map(|container| !container.state.running)
            .unwrap_or(true)
        {
            return false;
        }
    }
    true
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
