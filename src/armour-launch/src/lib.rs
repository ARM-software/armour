use armour_api::host::{OnboardInformation, Proxies};
use armour_compose::{Compose, OnboardInfo};
use armour_lang::{labels};
use awc::Client;
use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn onboard_services<P: AsRef<std::ffi::OsStr>>(
    client: Client,
    host_url: url::Url,
    info: OnboardInfo,
    out_file: P,
) -> Result<(), Error> {
    if let Some(host_str) = host_url.host_str() {
        let onboard_info: OnboardInformation = (&info).into();
        let url = format!(
            "https://{}:{}/service/on-board",
            host_str,
            host_url.port().unwrap_or(8090)
        );
        match client.post(url).send_json(&onboard_info).await {
            Ok(res) => {
                if res.status().is_success() {
                    println!("onboarding succeeded for {:#?}", info);
                    unpause_all(info).await;
                    Ok(())
                } else {
                    docker_down(out_file)?;
                    drop_services(client, host_url, info.proxies.clone()).await?;
                    Err(message(res)
                        .await
                        .unwrap_or_else(|| format!("onboarding failed for {:#?}", info))
                        .into())
                }
            }
            Err(e) => {
                docker_down(out_file)?;
                drop_services(client, host_url, info.proxies).await?;
                Err(format!("onboarding failed: {}", e).into())
            }
        }
    } else {
        Err(format!("onboarding failed, bad host URL: {}", host_url).into())
    }
}

pub async fn drop_services(
    client: Client,
    host_url: url::Url,
    proxies: Proxies,
) -> Result<(), Error> {
    if proxies.is_empty() {
        println!("no proxies to drop");
        Ok(())
    } else if let Some(host_str) = host_url.host_str() {
        let url = format!(
            "https://{}:{}/service/drop",
            host_str,
            host_url.port().unwrap_or(8090)
        );
        let res = client
            .delete(url)
            .send_json(&proxies.to_vec())
            .await
            .map_err(|err| format!("{}: drop services failed: {}", host_url, err))?;
        if res.status().is_success() {
            println!("drop services succeeded");
            Ok(())
        } else {
            Err(message(res)
                .await
                .unwrap_or_else(|| "drop services failed".to_string())
                .into())
        }
    } else {
        Err(format!("bad host URL: {}", host_url).into())
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
) -> Result<Vec<OnboardInfo>, Error> {
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

fn prerouting_rule(delete: bool, network_name: &str, port: u16) -> String {
    let mut s = format!(
        "iptables -t nat -{} PREROUTING -i {} -p tcp -j DNAT --to-destination 127.0.0.1:{}\n",
        if delete { "D" } else { "I" },
        network_name,
        port
    );
    if !delete {
        s.push_str(&format!(
            "sysctl -w net.ipv4.conf.{}.route_localnet=1\n",
            network_name
        ))
    }
    s
}

fn etc_hosts_rule(delete: bool, ip: std::net::Ipv4Addr, hostname: &str) -> String {
    if delete {
        format!("sed -i.bak '/{} {}/d' /etc/hosts\n", ip, hostname)
    } else {
        format!("echo '{} {}' >> /etc/hosts\n", ip, hostname)
    }
}

fn create_exe(stem: &std::ffi::OsStr, suffix: &str) -> std::io::Result<impl std::io::Write> {
    let mut file = stem.to_os_string();
    file.push(suffix);
    let mut file: std::path::PathBuf = file.into();
    file.set_extension("sh");
    let file = std::fs::File::create(file)?;
    let mut perms = file.metadata()?.permissions();
    perms.set_mode(0o744);
    file.set_permissions(perms)?;
    Ok(file)
}

pub fn rules(
    compose: armour_compose::Compose,
    onboard_info0: Vec<OnboardInfo>,//TODO build only one OnboardInformation with all needed things
    onboard_info: OnboardInformation,
    stem: &std::ffi::OsStr,
) -> Result<(), Error> {
    let mut up_file = create_exe(stem, "_up")?;
    let mut down_file = create_exe(stem, "_down")?;
    let mut hosts_file = create_exe(stem, "_hosts")?;
    let mut port_map = BTreeMap::new();
    let port = onboard_info.top_port();
    for proxy in onboard_info.proxies {
        let proxy_port = proxy.port(port);
        port_map.insert(proxy.label, proxy_port);
    }
    
    // PREROUTING DNAT rules for services
    for (service_name, service) in compose.services {
        if let armour_compose::network::Networks::Dict(dict) = &service.networks {
            if let Ok(service_label) = service_name.parse::<labels::Label>() {
                for (network_name, network) in dict {
                    let proxy_port: Option<&u16> = if port_map.len() == 1 {
                        port_map.values().next()
                    } else {
                        let proxy_name = onboard_info0.iter()//FIXME simplify
                            .filter(|x| x.services.get(&service_name).is_some())
                            .map(|x| x.proxies.iter().nth(0).unwrap() )
                            .nth(0).unwrap().clone().label;
                        port_map.get(&proxy_name)
                    };
                    if let Some(proxy_port) = proxy_port {
                        let s = format!("# {}\n", service_name);
                        let bytes = s.as_bytes();
                        up_file.write_all(bytes)?;
                        down_file.write_all(bytes)?;
                        up_file.write_all(
                            prerouting_rule(false, network_name, *proxy_port).as_bytes(),
                        )?;
                        down_file.write_all(
                            prerouting_rule(true, network_name, *proxy_port).as_bytes(),
                        )?
                    }
                    if let (Some(ip), Some(hostname)) =
                        (network.ipv4_address, service.hostname.as_ref())
                    {
                        hosts_file.write_all(etc_hosts_rule(false, ip, hostname).as_bytes())?;
                        down_file.write_all(etc_hosts_rule(true, ip, hostname).as_bytes())?
                    }
                }
            }
        }
    }
    println!(
        "generated files: {0}_up.sh, {0}_down.sh, {0}_hosts.sh",
        stem.to_string_lossy()
    );
    Ok(())
}
