use armour_launch::{
    already_running, docker_down, docker_up, drop_services, onboard_services, read_armour, rules,
    set_ip_addresses,
};
use clap::{crate_version, App, AppSettings, Arg, SubCommand};

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
    } else if let Some(rules_matches) = matches.subcommand_matches("rules") {
        let (compose, info) = armour_compose::Compose::read_armour(in_file)?;
        let rules_file = rules_matches.value_of("rules file").unwrap_or("rules");
        rules(compose, (&info).into(), std::ffi::OsStr::new(rules_file))
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
                .arg(Arg::with_name("rules file").required(false)),
        )
        .get_matches()
}
