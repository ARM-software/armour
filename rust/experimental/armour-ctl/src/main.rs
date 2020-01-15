use armour_policy::lang::Program;
use clap::{crate_version, App, Arg};
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Serialize, Deserialize)]
struct OnboardingData<'a> {
    label: &'a str,
    policy: Program,
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
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

    let policy = Program::from_file_option(matches.value_of("input file"))?;
    println!("{:?}", serde_json::to_string(&policy).unwrap());
    // policy.print();

    let url = matches
        .value_of("cplaneurl")
        .unwrap_or("http://localhost:8088/controlplane/onboarding");

    // label is compulsory, no request is made if there is no label
    if let Some(label) = matches.value_of("label") {
        println!("{}/{}", url, label);
        awc::Client::new()
            // .get((url.to_string() + "/" + label)) // <- Create request builder
            .post(url) // <- Create request builder
            .header("User-Agent", "Actix-web")
            .send_json(&OnboardingData { label, policy }) // <- Send http request
            .await
            .map(|response| {
                println!("Response: {:?}", response);
            })
            .unwrap_or_default()
    } else {
        eprintln!("No label provided")
    }

    Ok(())
}
