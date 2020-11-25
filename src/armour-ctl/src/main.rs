use armour_api::control;
use armour_lang::labels::Label;
use armour_lang::literals::CPID;
use armour_lang::policies;
use armour_utils::parse_https_url;
use clap::{crate_version, App};
use std::collections::{BTreeSet};
use std::str::FromStr;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    let yaml = clap::load_yaml!("../resources/cli.yml");
    let matches = App::from_yaml(yaml).version(crate_version!()).get_matches();

    let cp_url = parse_https_url(
        matches
            .value_of("CONTROLPLANEURL")
            .unwrap_or(control::CONTROL_PLANE),
        8088,
    )?;
    let host = cp_url.host_str().unwrap();
    let port = cp_url.port().unwrap();
    let url = |s: &str| format!("https://{}:{}/{}", host, port, s);

    // enable logging
    std::env::set_var("RUST_LOG", "armour_utils=info,armour_ctl=info");
    std::env::set_var("RUST_BACKTRACE", "0");
    env_logger::init();

    // build client for HTTPS connections
    let ca = matches
        .value_of("CA")
        .unwrap_or("certificates/armour-ca.pem");
    let certificate_password = matches.value_of("CERTIFICATE_PASSWORD").unwrap_or("armour");
    let certificate = matches
        .value_of("CERTIFICATE")
        .unwrap_or("certificates/armour-ctl.p12");
    let client = armour_utils::client(ca, certificate_password, certificate)?;

    // Request to update a policy
    if let Some(update_matches) = matches.subcommand_matches("update") {
        let file = update_matches.value_of("POLICYFILE").unwrap();
        let service = update_matches.value_of("SERVICE").unwrap();
        let labels = labels(update_matches);

        let client = {
            let policy = policies::Policies::from_file(file)?;
            let update_payload = control::PolicyUpdateRequest {
                label: service.parse().unwrap(),
                policy,
                labels,
            };
            client
            .post(url("policy/update"))
            .send_json(&update_payload)
            .await
        };

        match client {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    }
    else if let Some(update_matches) = matches.subcommand_matches("update-global") {
        let file = update_matches.value_of("POLICYFILE").unwrap();
        let labels = labels(update_matches);

        let client = {
            let selector : Option<Label> = match update_matches.value_of("SELECTOR") {
                Some(x) => Some(x.parse().unwrap()),
                _ => {
                    //by default all onboarding services are concerned
                    Some(Label::from_str("ServiceID::**").unwrap())
                }
            };

            println!("updating global policy");
            let policy = policies::Policies::from_file(file)?;
            let update_payload = control::CPPolicyUpdateRequest {
                label: control::global_policy_label(),
                policy,
                labels,
                selector,
            };
            client
            .post(url("policy/update-global"))
            .send_json(&update_payload)
            .await
        };

        match client {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    }
    else if let Some(update_matches) = matches.subcommand_matches("update-onboarding") {
        let file = update_matches.value_of("POLICYFILE").unwrap();
        let labels = labels(update_matches);

        let client = {
            println!("updating onboarding policy");
            let policy = policies::OnboardingPolicy::from_file(file)?;

            let update_payload = control::OnboardingUpdateRequest {
                label: control::onboarding_policy_label(),
                policy,
                labels,
            };
            client
            .post(url("policy/update-onboarding"))
            .send_json(&update_payload)
            .await
        };

        match client {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    }
    // Request to query a policy
    else if let Some(query_matches) = matches.subcommand_matches("query") {
        let service = query_matches.value_of("SERVICE").unwrap();
        let query_payload = control::PolicyQueryRequest {
            label: service.parse()?,
        };

        match client
            .get(url("policy/query"))
            .send_json(&query_payload)
            .await
        {
            Ok(mut response) => {
                let body = response.body().await.map_err(|_| "Payload error")?;
                if response.status().is_success() {
                    let req: armour_api::control::PolicyUpdateRequest =
                        serde_json::from_slice(body.as_ref())?;
                    println!("{}", req.policy);
                    println!("labels: {:?}", req.labels)
                } else {
                    println!("{}", string_from_bytes(body))
                }
            }
            Err(err) => println!("{}", err),
        }
    }
    else if let Some(_) = matches.subcommand_matches("query-global") {
        let query_payload = control::PolicyQueryRequest {
            label: control::global_policy_label(),
        };

        match client
            .get(url("policy/query-global"))
            .send_json(&query_payload)
            .await
        {
            Ok(mut response) => {
                let body = response.body().await.map_err(|_| "Payload error")?;
                if response.status().is_success() {
                    let req: armour_api::control::CPPolicyUpdateRequest =
                        serde_json::from_slice(body.as_ref())?;
                    println!("{}", req.policy);
                    println!("labels: {:?}", req.labels)
                } else {
                    println!("{}", string_from_bytes(body))
                }
            }
            Err(err) => println!("{}", err),
        }
    }
    else if let Some(_) = matches.subcommand_matches("query-onboarding") {
        let query_payload = control::PolicyQueryRequest {
            label: control::onboarding_policy_label(),
        };

        match client
            .get(url("policy/query-onboarding"))
            .send_json(&query_payload)
            .await
        {
            Ok(mut response) => {
                let body = response.body().await.map_err(|_| "Payload error")?;
                if response.status().is_success() {
                    let req: armour_api::control::OnboardingUpdateRequest =
                        serde_json::from_slice(body.as_ref())?;
                    println!("{}", req.policy);
                    println!("labels: {:?}", req.labels)
                } else {
                    println!("{}", string_from_bytes(body))
                }
            }
            Err(err) => println!("{}", err),
        }
    }
    // drop
    else if let Some(drop_matches) = matches.subcommand_matches("drop") {
        let service = drop_matches.value_of("SERVICE").unwrap();
        let drop_payload = control::PolicyQueryRequest {
            label: service.parse()?,
        };
        match client
            .delete(url("policy/drop"))
            .send_json(&drop_payload)
            .await
        {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    }
    else if let Some(_) = matches.subcommand_matches("drop-global") {
        let drop_payload = control::PolicyQueryRequest {
            label: control::global_policy_label(),
        };
        match client
            .delete(url("policy/drop"))
            .send_json(&drop_payload)
            .await
        {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    }
    else if let Some(_) = matches.subcommand_matches("drop-onboarding") {
        let drop_payload = control::PolicyQueryRequest {
            label: control::onboarding_policy_label(),
        };
        match client
            .delete(url("policy/drop"))
            .send_json(&drop_payload)
            .await
        {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    }
    // drop all
    else if matches.subcommand_matches("drop-all").is_some() {
        match client.delete(url("policy/drop-all")).send().await {
            Ok(response) => println!("success: {}", response.status().is_success()),
            Err(err) => println!("{}", err),
        }
    } 
    // Display specialization
    else if let Some(specialize_matches) = matches.subcommand_matches("specialize") {
        let host = specialize_matches.value_of("HOST").unwrap();
        let proxy = specialize_matches.value_of("PROXY").unwrap();
        let service = specialize_matches.value_of("SERVICE").unwrap();
        let file = specialize_matches.value_of("POLICYFILE").unwrap();
        let policy = policies::GlobalPolicies::from_file(file)?;
        let labels = specialize_matches.values_of("LABELS").unwrap().map(|s| s.parse::<Label>().unwrap());

        let specialize_payload = control::SpecializationRequest{
            host: host.parse()?,
            proxy: proxy.parse()?,
            service: service.parse()?,
            policy: policy,
            cpid: CPID::new(
                BTreeSet::default(),
                BTreeSet::default(),
                None,
                labels.into_iter().collect(),
            )
        };
        println!("Specialization request {:?}", specialize_payload);

        match client
            .post(url("policy/specialize"))
            .send_json(&specialize_payload)
            .await
        {
            Ok(mut response) => {
                let body = response.body().await.map_err(|_| "Payload error")?;
                if response.status().is_success() {
                    let req: armour_api::control::SpecializationResponse =
                        serde_json::from_slice(body.as_ref())?;
                    println!("{}", req.policy);
                } else {
                    println!("{}", string_from_bytes(body))
                }
            }
            Err(err) => println!("{}", err),
        }

    }
    if let Some(list_matches) = matches.subcommand_matches("list") {
        let path = match list_matches.value_of("ENTITY").unwrap() {
            "hosts" => "host/list",
            "services" => "service/list",
            "policies" => "policy/list",
            _ => unreachable!(),
        };
        match client.get(url(path)).send().await {
            Ok(mut response) => {
                let body = response.body().await.map_err(|_| "Payload error")?;
                if body.is_empty() {
                    println!("<none>")
                } else {
                    print!("{}", string_from_bytes(body))
                }
            }
            Err(err) => println!("{}", err),
        }
    }

    Ok(())
}

fn labels(matches: &clap::ArgMatches) -> control::LabelMap {
    let mut labels = control::LabelMap::new();
    if let Some(labelling) = matches.values_of("LABELS") {
        for (url, label) in labelling
            .clone()
            .step_by(2)
            .zip(
                labelling
                    .skip(1)
                    .step_by(2)
                    .map(|s| s.parse::<Label>().ok()),
            )
            .filter_map(|(url, l)| l.map(|l| (url.to_string(), l)))
        {
            if let Some(labels) = labels.get_mut(&url) {
                labels.insert(label);
            } else {
                labels.insert(url, label.into());
            }
        }
    }
    labels
}

fn string_from_bytes(b: bytes::Bytes) -> String {
    std::str::from_utf8(b.as_ref())
        .unwrap_or_default()
        .to_string()
}
