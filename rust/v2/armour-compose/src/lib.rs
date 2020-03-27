use armour_api::master::OnboardInformation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap as Map;
use std::{fs, io, path};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Compose {
    pub version: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub services: Map<String, service::Service>,

    #[serde(default)]
    pub networks: Map<String, network::Network>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Map::is_empty")]
    volumes: Map<String, Option<volume::VolumeConfig>>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Map::is_empty")]
    configs: Map<String, Option<config::ConfigConfig>>,

    #[serde(default)]
    #[serde(skip_serializing)]
    secrets: Map<String, Option<secret::SecretConfig>>,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

impl Compose {
    pub fn from_path<P: AsRef<path::Path>>(p: P) -> Result<Self, serde_yaml::Error> {
        let file = fs::File::open(p).map_err(serde::de::Error::custom)?;
        serde_yaml::from_reader(io::BufReader::new(file))
    }
    pub fn read_armour<P: AsRef<path::Path>>(
        p: P,
    ) -> Result<(Self, OnboardInformation), Box<dyn std::error::Error + Send + Sync>> {
        let mut compose = Compose::from_path(p)?;
        let info = compose.convert_for_armour()?;
        Ok((compose, info))
    }
    fn convert_for_armour(&mut self) -> Result<OnboardInformation, String> {
        let mut info = OnboardInformation::new();
        let mut networks = Map::new();
        for (name, service) in self.services.iter_mut() {
            if name.len() > 12 {
                return Err(format!("service name too long, max 12 chars: {}", name));
            };
            let (service_info, network) = service.convert_for_armour(name);
            info.insert(name.to_string(), service_info);
            networks.insert(service::Service::armour_bridge_network(name), network);
        }
        self.networks = networks;
        Ok(info)
    }
    pub fn validate() -> bool {
        true
    }
}

impl std::str::FromStr for Compose {
    type Err = serde_yaml::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_yaml::from_str(s)
    }
}

mod capabilities;
mod config;
mod secret {
    pub type SecretConfig = super::config::ConfigConfig;
    pub type Secret = super::config::Config;
}
pub mod network;
pub mod service;
mod volume;

mod external {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ExternalRecord {
        name: String,
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    #[serde(untagged)]
    pub enum External {
        Raw(bool),
        Struct(ExternalRecord),
    }

    impl Default for External {
        fn default() -> Self {
            External::Raw(false)
        }
    }
}
