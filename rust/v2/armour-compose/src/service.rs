use super::serde_utils::{array_dict, from_str, is_default, string_or_list, string_or_struct};
use super::{capabilities, config, network, secret, volume};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap as Map;
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
pub struct Service {
    // TODO:
    //
    // credential_check
    // healthcheck
    // logging
    // ulimits
    //
    #[serde(default)]
    #[serde(deserialize_with = "string_or_struct::deserialize")]
    #[serde(skip_serializing_if = "is_default")]
    build: Build,

    #[serde(skip_serializing_if = "Option::is_none")]
    cgroup_parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    domainname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ipc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    isolation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mac_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shm_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_grace_period: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    userns_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    depends_on: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    devices: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    expose: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    external_links: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    security_opt: Vec<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    configs: Vec<config::Config>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    ports: Vec<Port>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    secrets: Vec<secret::Secret>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    volumes: Vec<volume::Volume>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    init: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    privileged: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    read_only: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    stdin_open: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    tty: bool,

    #[serde(flatten)]
    capabilities: capabilities::Capabilities,

    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    command: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    dns_search: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    entrypoint: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    env_file: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    tmpfs: string_or_list::StringList,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "string_or_list::deserialize")]
    dns: Vec<std::net::IpAddr>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    environment: array_dict::ArrayDict,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    extra_hosts: array_dict::ArrayDict,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    labels: array_dict::ArrayDict,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    sysctls: array_dict::ArrayDict,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    deploy: Deployment,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    networks: network::Networks,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

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

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all(serialize = "kebab-case"))]
pub enum Order {
    StartFirst,
    StopFirst,
}

impl std::str::FromStr for Order {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "start-first" => Ok(Order::StartFirst),
            "stop-first" => Ok(Order::StopFirst),
            _ => Err("failed to parse rollback order"),
        }
    }
}

deserialize_from_str!(Order);

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    parallelism: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delay: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_failure_ratio: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<Order>,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Resource {
    #[serde(skip_serializing_if = "Option::is_none")]
    cpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory: Option<String>,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Resources {
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    limits: Resource,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    reservations: Resource,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Deployment {
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint_mode: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    replicas: Option<usize>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    labels: array_dict::ArrayDict,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    rollback_config: Config,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    update_config: Config,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    resources: Resources,

    // TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    restart_policy: Option<serde_yaml::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    placement: Option<serde_yaml::Value>,

    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}
