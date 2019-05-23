use clap::{crate_version, App, Arg};
use docker_compose_yaml::compose::Compose;

fn main() {
  let matches = App::new("docker-compose-yaml")
    .version(crate_version!())
    .author("Anthony Fox <anthony.fox@arm.com>")
    .about("Deserialize and Serialize Docker Compose")
    .arg(
      Arg::with_name("input file")
        .index(1)
        .required(true)
        .help("input file"),
    )
    .get_matches();

  // #[allow(unused_variables)]
  // let test_string = r#"
  //       version: "3.7"
  //       services:
  //         s1:
  //           build: ./dir
  //           entrypoint: /bin/bash
  //           dns:
  //           - 8.8.8.8
  //           - 9.9.9.9
  //           ports:
  //           - 100
  //           - target: 80
  //             published: 8080
  //             protocol: tcp
  //             mode: host
  //           cap_add:
  //           - ALL
  //           cap_drop:
  //           - FOWNER
  //           - FSETID
  //           networks:
  //           - one
  //           - two
  //           - three
  //           expose:
  //           - 5000
  //         s2:
  //           build:
  //             context: ./dir
  //             args:
  //             - one
  //             - two
  //             target: t
  //           networks:
  //             one:
  //               aliases:
  //               - alias1
  //               - alias2
  //         s3:
  //           networks:
  //             two:
  //               aliases:
  //               - alias1
  //               - alias2
  //               ipv4_address: 1.1.1.1
  //               ipv6_address: 2001:3984:3989::10
  //             three:
  //           volumes:
  //             - one
  //             - type: volume
  //               source: mydata
  //               target: /data
  //               volume:
  //                 nocopy: true
  //           configs:
  //           - one
  //           - source: a
  //           - source: b
  //             target: c
  //             uid: 100
  //       networks:
  //         one:
  //           driver: OVerlay
  //           driver_opts:
  //             type: "a"
  //           ipam:
  //             driver: default
  //             config:
  //             - subnet: 172.28.0.0/16
  //             - subnet: 173.28.0.0/16
  //           labels:
  //             a: a
  //       volumes:
  //         data-volume:
  //         other-volume:
  //           external: true
  //         another-volume:
  //           external:
  //             name: name-of-volume
  //       configs:
  //           my_first_config:
  //             file: ./config_data
  //           my_second_config:
  //             external: true
  //           my_third_config:
  //             external:
  //               name: redis_config
  //   "#;

  if let Some(filename) = matches.value_of("input file") {
    match Compose::from_path(filename) {
      // match test_string.parse::<Compose>() {
      Ok(compose) => {
        // println!("{:#?}", compose);
        println!("{}", serde_yaml::to_string(&compose).unwrap())
      }
      Err(e) => println!("{}", e),
    }
  }
}
