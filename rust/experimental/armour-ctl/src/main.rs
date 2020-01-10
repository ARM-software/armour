use armour_policy::lang;
use clap::{crate_version, App, Arg};
use std::io;
use serde::{Serialize, Deserialize};

use armour_policy::lang::Program;

#[derive(Serialize, Deserialize)]
struct OnboardingData {
    label: String,
    policy: Program,
}


#[actix_rt::main]
async fn main() -> io::Result<()> {
    let matches = App::new("Armour Control Plane CLI")
        .version(crate_version!())
        .arg(
            Arg::with_name("policy")
                .index(1)
                .short("p")
                .long("policy")
                .takes_value(true)
                .required(true)
                .help("Policy file"),
        )
        .arg(
            Arg::with_name("label")
                .index(2)
                .short("l")
                .long("label")
                .takes_value(true)
                .required(true)
                .help("Endpoint Label"),
        )
        .arg(
            Arg::with_name("cplaneurl")
                .index(3)
                .short("c")
                .long("ctrplane")
                .takes_value(true)
                .required(false)
                .help("Control Plane URL"),
        )
        .get_matches();

    let module: lang::Module;
    
    // try to load code from an input file
    if let Some(filename) = matches.value_of("policy") {
        module = lang::Module::from_file(filename, None)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    } else {
        module = lang::Module::default()
    }
    let prog = module.program;
    prog.print();
    
    let url : &str;

    // control plane url is optional, default is localhost:8088
    if let Some(addr) = matches.value_of("cplaneurl") {
	url = addr
    } else {
	url = "http://localhost:8088/controlplane/onboarding"
    }

    // label is compulsory, no request is made if there is no label
    if let Some(label) = matches.value_of("label") {
	println!("{:?}", url.to_string() + "/" + label);
	let response = awc::Client::new()
	    // .get((url.to_string() + "/" + label)) // <- Create request builder
	    .post(url.to_string()) // <- Create request builder
	    .header("User-Agent", "Actix-web")
	    .send_json(&OnboardingData { label : label.to_string(), policy : prog.clone() } )                          // <- Send http request
	    .await;

	let _ = response.and_then(|response| {   // <- server http response
	    println!("Response: {:?}", response);
	    Ok(())
	});
    } else {
	eprintln!("No label provided");	
    }

    return io::Result::Ok(());
}
