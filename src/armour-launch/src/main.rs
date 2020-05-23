use armour_api::master::OnboardInformation;
use armour_launch::{
    already_running, docker_down, docker_up, drop_services, onboard_services, read_armour,
    set_ip_addresses,
};
use clap::{crate_version, App, AppSettings, Arg, SubCommand};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    let matches = get_matches();
    const TCP_PORT: u16 = 8090;
    let master_port = matches
        .value_of("port")
        .map(|s| s.parse::<u16>().unwrap_or(TCP_PORT))
        .unwrap_or(TCP_PORT);
    const FILE: &str = "docker-compose.yml";
    let out_file = matches.value_of("file").unwrap_or(FILE);
    let in_file = matches.value_of("input file").unwrap();
    if let Some(_up) = matches.subcommand_matches("up") {
        // read armour-compose from input file and write docker-compose.yml file
        let mut info = read_armour(in_file, out_file)?;
        // check if application is already running
        if already_running(&info).await {
            return Err("already running! run armour-lauch `down` first?".into());
        } else {
            // try to run `docker-compose up` command
            docker_up(out_file)?;
            // try to set IP addresses for containers (leaves containers in paused state)
            set_ip_addresses(&mut info).await;
            // notify data plane master - onboarding
            onboard_services(master_port, info, out_file).await
        }
    } else if let Some(_down) = matches.subcommand_matches("down") {
        // create docker-compose.yml from armour-compose input file
        let info = read_armour(in_file, out_file)?;
        // try to run `docker-compose down` command
        docker_down(out_file)?;
        drop_services(master_port, info.proxies).await
    } else if let Some(_rules) = matches.subcommand_matches("rules") {
        let (compose, info) = armour_compose::Compose::read_armour(in_file)?;
        let onboard_info: OnboardInformation = (&info).into();
        generate_rules(compose, onboard_info, std::ffi::OsStr::new("rules"))
    } else {
        unreachable!()
    }
}

fn forward_rule1(delete: bool, port: u16) -> String {
    format!(
        "iptables -{} FORWARD -p tcp -d localhost --dport {} -j ACCEPT\n",
        if delete { "D" } else { "A" },
        port
    )
}

fn forward_rule(delete: bool, ports: &str) -> String {
    format!(
        "iptables -{} FORWARD -p tcp -d localhost --match multiport --dports {} -j ACCEPT\n",
        if delete { "D" } else { "A" },
        ports
    )
}

fn prerouting_rule(delete: bool, network_name: &str, port: u16) -> String {
    let mut s = format!(
        "iptables -t nat -{} PREROUTING -i {} -p tcp -j DNAT --to-destination 127.0.0.1:{}\n",
        if delete { "D" } else { "I" },
        network_name,
        port
    );
    s.push_str(&format!(
        "sysctl -w net.ipv4.conf.{}.route_localnet={}\n",
        network_name,
        if delete { "0" } else { "1" }
    ));
    s
}

fn etc_hosts_rule(delete: bool, ip: std::net::Ipv4Addr, hostname: &str) -> String {
    if delete {
        format!("sed -i.bak '/{} {}/d' /etc/hosts\n", ip, hostname)
    } else {
        format!("echo '{} {}' >> /etc/hosts\n", ip, hostname)
    }
}

fn generate_rules(
    compose: armour_compose::Compose,
    onboard_info: OnboardInformation,
    stem: &std::ffi::OsStr,
) -> Result<(), Error> {
    let mut up_file = stem.to_os_string();
    up_file.push("_up");
    let mut up_file: std::path::PathBuf = up_file.into();
    up_file.set_extension("sh");
    let mut down_file = stem.to_os_string();
    down_file.push("_down");
    let mut down_file: std::path::PathBuf = down_file.into();
    down_file.set_extension("sh");
    let up_file = std::fs::File::create(up_file)?;
    let down_file = std::fs::File::create(down_file)?;
    let mut perms = up_file.metadata()?.permissions();
    perms.set_mode(0o744);
    up_file.set_permissions(perms.clone())?;
    down_file.set_permissions(perms)?;
    rules(compose, onboard_info, up_file, down_file)
}

fn rules<W: std::io::Write>(
    compose: armour_compose::Compose,
    onboard_info: OnboardInformation,
    mut up_file: W,
    mut down_file: W,
) -> Result<(), Error> {
    let mut port = onboard_info.top_port();
    let mut port_map = BTreeMap::new();
    for proxy in onboard_info.proxies {
        let proxy_port = proxy.port.unwrap_or_else(|| {
            port += 1;
            port
        });
        port_map.insert(proxy.label, proxy_port);
    }
    let ports: Vec<&u16> = port_map.values().collect();
    let one_proxy = ports.len() == 1;
    if one_proxy {
        let port = *ports[0];
        up_file.write_all(forward_rule1(false, port).as_bytes())?;
        down_file.write_all(forward_rule1(true, port).as_bytes())?;
    } else if !ports.is_empty() {
        let port_list = ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(",");
        up_file.write_all(forward_rule(false, &port_list).as_bytes())?;
        down_file.write_all(forward_rule(true, &port_list).as_bytes())?;
    }
    for (service_name, service) in compose.services {
        let s = format!("# {}\n", service_name);
        let bytes = s.as_bytes();
        up_file.write_all(bytes)?;
        down_file.write_all(bytes)?;
        if let armour_compose::network::Networks::Dict(dict) = &service.networks {
            if let Ok(service_label) = service_name.parse() {
                for (network_name, network) in dict {
                    let proxy_port: Option<&u16> = if one_proxy {
                        ports.get(0).copied()
                    } else {
                        port_map.get(&service_label)
                    };
                    if let Some(proxy_port) = proxy_port {
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
                        up_file.write_all(etc_hosts_rule(false, ip, hostname).as_bytes())?;
                        down_file.write_all(etc_hosts_rule(true, ip, hostname).as_bytes())?
                    }
                }
            }
        }
    }
    Ok(())
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
                .arg(Arg::with_name("rules file").required(true)),
        )
        .get_matches()
}
