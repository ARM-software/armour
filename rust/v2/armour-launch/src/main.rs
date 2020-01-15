use clap::{crate_version, App, Arg};

fn main() {
    let matches = App::new("docker-compose-yaml")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com>")
        .about("Deserialize and Serialize Docker Compose")
        .arg(
            Arg::with_name("input file")
                .index(1)
                .required(false)
                .help("input file"),
        )
        .get_matches();

    if let Some(filename) = matches.value_of("input file") {
        match armour_compose::Compose::from_path(filename) {
            Ok(compose) => {
                // println!("{:#?}", compose);
                println!("{}", serde_yaml::to_string(&compose).unwrap())
            }
            Err(e) => println!("{}", e),
        }
    }
}
