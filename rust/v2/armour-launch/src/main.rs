use clap::{crate_version, App, Arg, SubCommand};
use std::collections::BTreeMap as Map;
use std::fs;
use std::process::{Command};
use std::io::{Error, ErrorKind};
use awc::Client;


#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let matches = App::new("armour-compose")
        .version(crate_version!())
        .author("Anthony Fox <anthony.fox@arm.com> and Basma El Gaabouri <basma.elgaabouri@arm.com>")
        .about("Armour launcher")
        .subcommand(SubCommand::with_name("up")
            .about("Start Armour compose")
            .version(crate_version!())
            .arg(Arg::with_name("input file")
                .index(1)
                .help("Start armour compose")))
        .subcommand(SubCommand::with_name("down")
            .about("Stop Armour compose")
            .version(crate_version!())
            .arg(Arg::with_name("input file")
                .index(1)
                .help("Stop armour compose")))
        .get_matches();

    if let Some(up) = matches.subcommand_matches("up") {
        if let filename = up.value_of("input file").unwrap_or("armour-compose.yml") {
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
                        let temp: armour_compose::service::Armour = compose.services.get(&service).unwrap().armour.clone();
                        info.insert(service.to_string(), temp);

                        let mut driver_opt = Map::new();
                        let mut name: String = "arm-".to_owned();
                        name.push_str(&service.to_string());
                        driver_opt.insert("com.docker.network.bridge.name".to_string(),name.clone());
                        let net: armour_compose::network::Network = armour_compose::network::Network { driver: Some(armour_compose::network::Driver::Bridge), driver_opts: driver_opt, internal: true, ..Default::default()};
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
                    
                    //get output of this command
                    let cmd = Command::new("docker-compose up --no-start")
                                //.arg("ls")
                                //.arg("docker-compose up --no-start")
                                .output().unwrap_or_else(|e| {
                                    panic!("failed to execute process: {}", e)
                            });
                    if cmd.status.success() {
                        let s = String::from_utf8_lossy(&cmd.stdout);
                        
                        print!("docker-compose succeeded and stdout was:\n{}", s);
                    } else {
                        let s = String::from_utf8_lossy(&cmd.stderr);
                        
                        print!("docker-compose failed and stderr was:\n{}", s);
                    }    
                    println!("done with cmd");      
                                
                                //.status().expect("docker-compose command failed to execute");
                    //let stat = cmd.stdout;
                    //println!("{:?}", stat);
                    //match Command::new("docker-compose")
                    //          .arg("up --no-start")
                        //        .stdout(Stdio::piped())
                        //      .spawn()
                                //.expect("failed to execute process") 
                    //{
                    //   Ok(child) => {
                    //  }
                    //    Err(e) => { println!("{}", e);}
                    //}
                    
                    //gets the exit code .. but i want the fucking error
                // if cmd.status.success() {
                        //apparently not catching the error
                    //    return Err(Error::new(ErrorKind::Other, "something went wrong"));
                    //} 
                // io::stderr().write_all(&output).unwrap();
                // println!("{}", output.status);
                    //println!("this is {}", String::from_utf8_lossy(&output.stdout));
                    
                    //cmd.arg("-c").arg("docker-compose logs").status()?;
                    //cmd.status().expect("docker-compose command failed to execute");

                //  if {
                    //    return Err(Error)
                // }
                    //match cmd.stderr(Stdio::null()).status().expect() {
                    //   Err(e) => println!("got an error"),
                    //  Ok(..) => println!("works"),
                // }
                    
                    //.expect("error starting containers");
                    
                    
                    //should the created networks be sents with the labels ?
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
                            //cmd.arg("-c").arg("docker-compose up");
                            //cmd.status().expect("process failed to execute")
                        },
                        Err(_error) => { panic!("failed to get a successful response status!"); }
                    };
                    //if !response.status().is_success() {
                    //panic!("failed to get a successful response status!");
                    //}
                    println!("end");

                }
                Err(e) => println!("{}", e),
            }
        }
    } else if let Some(down) = matches.subcommand_matches("down") {
        if let filename = down.value_of("input file").unwrap_or("docker-compose.yml") {
            match armour_compose::Compose::from_path(filename) {
                Ok(compose) => {
                    println!("processing down subcommand .. ");


                }
                Err(e) => println!("{}", e),
            }
        }
    }
    Ok(())
}