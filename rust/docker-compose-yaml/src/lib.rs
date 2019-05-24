#[macro_use]
extern crate lazy_static;

#[macro_use]
mod serde_utils;
mod capabilities;
mod config;
mod secret {
    pub type SecretConfig = super::config::ConfigConfig;
    pub type Secret = super::config::Config;
}
mod network;
mod service;
mod volume;

pub mod compose {
    use super::{config, network, secret, service, volume};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap as Map;
    use std::{fs, io, path};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Compose {
        version: String,

        #[serde(default)]
        #[serde(skip_serializing_if = "Map::is_empty")]
        services: Map<String, service::Service>,

        #[serde(default)]
        #[serde(skip_serializing_if = "Map::is_empty")]
        networks: Map<String, network::Network>,

        #[serde(default)]
        #[serde(skip_serializing_if = "Map::is_empty")]
        volumes: Map<String, Option<volume::VolumeConfig>>,

        #[serde(default)]
        #[serde(skip_serializing_if = "Map::is_empty")]
        configs: Map<String, Option<config::ConfigConfig>>,

        #[serde(default)]
        #[serde(skip_serializing_if = "Map::is_empty")]
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
    }

    impl std::str::FromStr for Compose {
        type Err = serde_yaml::Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            serde_yaml::from_str(s)
        }
    }
}
mod external {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ExternalRecord {
        name: String,
    }

    #[derive(Deserialize, Serialize, Debug)]
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
