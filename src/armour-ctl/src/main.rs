use armour_api::{control, master};
use armour_lang::lang;
use awc::http;
#[macro_use]
extern crate clap;
use clap::{crate_version, App};
use json::JsonValue;
use std::io::{Error, ErrorKind};
use url::Url;

const DEFAULT_CONTROL_PLANE: &str = "http://127.0.0.1:8088/controlplane";

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let yaml = load_yaml!("../resources/cli.yml");
    let matches = App::from_yaml(yaml).version(crate_version!()).get_matches();

    let cp_url = matches
        .value_of("CONTROLPLANEURL")
        .unwrap_or(DEFAULT_CONTROL_PLANE);

    let client = awc::Client::build().finish();

    // Request to update a policy
    if let Some(update_matches) = matches.subcommand_matches("update") {
        let file = update_matches
            .value_of("POLICYFILE")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong policy file"))?;
        let service = update_matches
            .value_of("SERVICE")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong service"))?;
        let prog = lang::Program::from_file(file, None)?;
        let update_payload = control::PolicyUpdateRequest {
            label: service.parse().unwrap(),
            policy: master::Policy::Bincode(prog.to_bincode()?),
            labels: control::LabelMap::new(),
        };

        let req = client
            .post(cp_url.to_owned() + "/update-policy")
            .header(http::header::CONTENT_TYPE, "application/json")
            .send_json(&update_payload);
        let r = req.await;
        println!("{:?}", r.unwrap());
    }

    // Request to query a policy
    if let Some(query_matches) = matches.subcommand_matches("query") {
        let service = query_matches
            .value_of("SERVICE")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong service"))?;

        let query_payload = control::PolicyQueryRequest {
            label: service.parse().unwrap(),
        };

        let mut req = client
            .get(cp_url.to_owned() + "/query-policy")
            .header(http::header::CONTENT_TYPE, "application/json")
            .send_json(&query_payload)
            .await
            .map_err(|_| Error::new(ErrorKind::Other, "Server error"))?;
        let body = req
            .body()
            .await
            .map_err(|_| Error::new(ErrorKind::Other, "Server error"))?;
        match json::parse(
            std::str::from_utf8(&body)
                .map_err(|_| Error::new(ErrorKind::Other, "Parse policy error"))?,
        )
        .map_err(|_| Error::new(ErrorKind::Other, "Parse policy error"))?
        {
            JsonValue::String(s) => {
                println!("{}", lang::Program::from_bincode_raw(s.as_bytes()).unwrap())
            }
            _ => panic!("wrong string"),
        };
    }

    // Request to onboard a master
    if let Some(master_matches) = matches.subcommand_matches("fake-master-onboard") {
        let master_url = master_matches
            .value_of("MASTERURL")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong master URL"))?;
        let masterlabel = master_matches
            .value_of("MASTER")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong master label"))?;

        let master_payload = control::OnboardMasterRequest {
            host: Url::parse(&master_url.to_owned())
                .map_err(|_| Error::new(ErrorKind::Other, "Wrong master label"))?,
            master: masterlabel.parse().unwrap(),
            credentials: "No Credential".to_string(),
        };

        let req = client
            .post(cp_url.to_owned() + "/onboard-master")
            .header(http::header::CONTENT_TYPE, "application/json")
            .send_json(&master_payload);
        let r = req.await;
        println!("{:?}", r.unwrap());
    }

    // Request to onboard a service
    if let Some(service_matches) = matches.subcommand_matches("fake-service-onboard") {
        let service = service_matches
            .value_of("SERVICE")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong service"))?;
        let master = service_matches
            .value_of("MASTER")
            .ok_or_else(|| Error::new(ErrorKind::Other, "Wrong master"))?;

        let service_payload = control::OnboardServiceRequest {
            service: service.parse().unwrap(),
            master: master.parse().unwrap(),
        };

        let req = client
            .post(cp_url.to_owned() + "/onboard-service")
            .header(http::header::CONTENT_TYPE, "application/json")
            .send_json(&service_payload);
        let r = req.await;
        println!("{:?}", r.unwrap());
    }

    Ok(())
}
