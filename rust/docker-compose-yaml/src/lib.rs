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

pub mod build {
    use super::serde_utils::{array_dict, is_default};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap as Map;

    #[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
    pub struct Build {
        context: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dockerfile: Option<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "is_default")]
        args: array_dict::ArrayDict,
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        cache_from: Vec<String>,
        #[serde(default)]
        #[serde(skip_serializing_if = "is_default")]
        labels: array_dict::ArrayDict,
        #[serde(skip_serializing_if = "Option::is_none")]
        shm_size: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        // capture everything else (future proofing)
        #[serde(skip_serializing)]
        #[serde(flatten)]
        _extras: Map<String, serde_yaml::Value>,
    }

    impl std::str::FromStr for Build {
        type Err = &'static str;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Build {
                context: s.to_string(),
                dockerfile: None,
                args: array_dict::ArrayDict::default(),
                cache_from: Vec::new(),
                labels: array_dict::ArrayDict::default(),
                shm_size: None,
                target: None,
                _extras: Map::new(),
            })
        }
    }
}

mod ports {
    use serde::de::{self, MapAccess, Visitor};
    use serde::{Deserialize, Deserializer, Serialize};
    use std::collections::BTreeMap as Map;
    use std::fmt;

    #[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
    pub struct PortRecord {
        target: u16,
        published: u16,
        protocol: String,
        mode: String,
        #[serde(skip_serializing)]
        #[serde(flatten)]
        _extras: Map<String, serde_yaml::Value>,
    }

    #[derive(Serialize, Debug)]
    #[serde(untagged)]
    pub enum Port {
        Raw(String),
        Struct(PortRecord),
    }

    impl<'de> Deserialize<'de> for Port {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct PortVisitor;

            impl<'de> Visitor<'de> for PortVisitor {
                type Value = Port;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("string or map")
                }

                fn visit_u64<E>(self, value: u64) -> Result<Port, E>
                where
                    E: de::Error,
                {
                    Ok(Port::Raw(value.to_string()))
                }

                fn visit_str<E>(self, value: &str) -> Result<Port, E>
                where
                    E: de::Error,
                {
                    Ok(Port::Raw(value.to_owned()))
                }

                fn visit_map<V>(self, map: V) -> Result<Port, V::Error>
                where
                    V: MapAccess<'de>,
                    V::Error: de::Error,
                {
                    Ok(Port::Struct(
                        PortRecord::deserialize(de::value::MapAccessDeserializer::new(map))
                            .map_err(de::Error::custom)?,
                    ))
                }
            }

            deserializer.deserialize_any(PortVisitor)
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
