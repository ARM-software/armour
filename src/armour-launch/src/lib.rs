use armour_api::master::{OnboardInformation, Proxies};
use armour_compose::{Compose, OnboardInfo};
use awc::Client;
use std::process::Command;

type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn onboard_services<P: AsRef<std::ffi::OsStr>>(
    master_port: u16,
    info: OnboardInfo,
    out_file: P,
) -> Result<(), Error> {
    let client = Client::default();
    let onboard_info: OnboardInformation = (&info).into();
    match client
        .post(format!("http://localhost:{}/service/on-board", master_port))
        .send_json(&onboard_info)
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                println!("onboarding succeeded");
                unpause_all(info).await;
                Ok(())
            } else {
                docker_down(out_file)?;
                drop_services(master_port, info.proxies).await?;
                Err(message(res)
                    .await
                    .unwrap_or_else(|| "onboarding failed".to_string())
                    .into())
            }
        }
        Err(e) => {
            docker_down(out_file)?;
            drop_services(master_port, info.proxies).await?;
            Err(format!("onboarding failed: {}", e).into())
        }
    }
}

pub async fn drop_services(master_port: u16, proxies: Proxies) -> Result<(), Error> {
    if proxies.is_empty() {
        println!("no proxies to drop");
        Ok(())
    } else {
        let client = Client::default();
        let res = client
            .delete(format!("http://localhost:{}/service/drop", master_port))
            .send_json(&proxies.to_vec())
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

pub fn docker_up<P: AsRef<std::ffi::OsStr>>(out_file: P) -> Result<(), Error> {
    let child = Command::new("docker-compose")
        .arg("--file")
        .arg(out_file)
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

pub fn docker_down<P: AsRef<std::ffi::OsStr>>(out_file: P) -> Result<(), Error> {
    let child = Command::new("docker-compose")
        .arg("--file")
        .arg(out_file)
        .arg("down")
        .output()?;
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

pub fn read_armour<P: AsRef<std::path::Path>>(
    in_file: P,
    out_file: P,
) -> Result<OnboardInfo, Error> {
    // load armour compose file
    let (compose, info) = Compose::read_armour(in_file)?;
    // save as docker compose file
    std::fs::write(out_file, serde_yaml::to_string(&compose)?)?;
    Ok(info)
}

async fn unpause_all(information: OnboardInfo) {
    // try to get IP addresses for containers
    let docker = docker_api::Docker::new();
    for name in information.services.keys() {
        if let Err(e) = docker.unpause_container(name).await {
            println!("warn: {}", e)
        } else {
            println!("unpaused: {}", name)
        }
    }
}

pub async fn already_running(information: &OnboardInfo) -> bool {
    // try to get IP addresses for containers
    let docker = docker_api::Docker::new();
    for name in information.services.keys() {
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

pub async fn set_ip_addresses(information: &mut OnboardInfo) {
    // try to get IP addresses for containers
    let docker = docker_api::Docker::new();
    for (name, info) in information.services.iter_mut() {
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
