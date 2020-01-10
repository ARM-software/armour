use clap::{crate_version, App, Arg};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Serialize, Deserialize)]
pub struct OnboardMasterRequest {
    pub host: String,        // FIXME change types as needed
    pub credentials: String, // FIXME change types as needed
}

#[actix_rt::main]
async fn main() -> io::Result<()> {
    let matches = App::new("Armour Fake Data Plane Master")
	.version(crate_version!())
	.arg(Arg::with_name("cplaneurl")
	     .index(1)
	     .short("c")
	     .long("ctrplane")
	     .takes_value(true)
	     .required(true)
	     .help("Control Plane URL"))
	.arg(Arg::with_name("hostname")
	     .index(2)
	     .short("h")
	     .long("hostname")
	     .takes_value(true)
	     .required(false)
	     .help("Local host name"))
	.get_matches();

    let url: &str;

    // control plane url is optional, default is localhost:8088
    if let Some(addr) = matches.value_of("cplaneurl") {
	url = addr
    } else {
	url = "http://localhost:8088"
    }

    let host: &str;
    if let Some(hst) = matches.value_of("hostname") {
	host = hst;
    } else {
	host = "localhost";
    }

    println!("{:?}", url.to_string() + &"/controlplane/onboard-master".to_string());
    
    let response = awc::Client::new()
	.post(url.to_string() + &"/controlplane/onboard-master".to_string()) // <- Create request builder
	.header("User-Agent", "Actix-web")
	.send_json(&OnboardMasterRequest {
	    host: host.to_string(),
	    credentials: "no-cerds".to_string(),
	}) // <- Send http request
	.await;

    let _ = response.and_then(|response| {
	// <- server http response
	println!("Response: {:?}", response);
	Ok(())
    });

    return io::Result::Ok(());
}
