use super::serde_utils::array_dict;
use super::serde_utils::{from_str, is_default};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap as Map;
use std::fmt;

#[derive(Serialize, Debug)]
#[serde(rename_all(serialize = "lowercase"))]
pub enum Driver {
    Bridge,
    Overlay,
    Host,
}

impl std::str::FromStr for Driver {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "bridge" => Ok(Driver::Bridge),
            "overlay" => Ok(Driver::Overlay),
            "host" => Ok(Driver::Host),
            _ => Err("failed to parse driver"),
        }
    }
}

deserialize_from_str!(Driver);

#[derive(Serialize, Deserialize, Debug)]
pub struct IpamConfig {
    subnet: ipnetwork::IpNetwork,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Ipam {
    driver: String,
    config: Vec<IpamConfig>,
    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Network {
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    driver: Option<Driver>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Map::is_empty")]
    driver_opts: Map<String, String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ipam: Option<Ipam>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    external: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    internal: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    attachable: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    labels: array_dict::ArrayDict,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct NetworkRecord {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ipv4_address: Option<std::net::Ipv4Addr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ipv6_address: Option<std::net::Ipv6Addr>,
    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(untagged)]
pub enum Networks {
    Array(Vec<String>),
    Dict(Map<String, NetworkRecord>),
}

impl Default for Networks {
    fn default() -> Networks {
        Networks::Array(Vec::new())
    }
}

impl<'de> Deserialize<'de> for Networks {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NetworksVisitor;

        impl<'de> Visitor<'de> for NetworksVisitor {
            type Value = Networks;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string list or map")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Networks, A::Error>
            where
                A: SeqAccess<'de>,
                A::Error: de::Error,
            {
                let mut v = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(value) = seq.next_element()? {
                    v.push(value);
                }
                Ok(Networks::Array(v))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Networks, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut m = Map::new();
                while let Some((key, value)) = map.next_entry::<String, Option<NetworkRecord>>()? {
                    m.insert(key, value.unwrap_or_default());
                }
                Ok(Networks::Dict(m))
            }
        }

        deserializer.deserialize_any(NetworksVisitor)
    }
}
