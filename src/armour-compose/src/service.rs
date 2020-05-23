use super::{capabilities, config, network, secret, volume, ServiceInfo};
use armour_serde::{
    array_dict, deserialize_from_str, is_default, string_or_list, string_or_struct,
};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap as Map;
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Service {
    // TODO:
    //
    // credential_check
    // healthcheck
    // logging
    // ulimits
    #[serde(default)]
    #[serde(deserialize_with = "string_or_struct::deserialize")]
    #[serde(skip_serializing_if = "is_default")]
    pub build: Build,

    //#[serde(skip_serializing_if = "skip")]
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    //#[serde(skip_deserializing)]
    pub armour: Armour,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroup_parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domainname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shm_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_grace_period: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userns_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expose: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub external_links: Vec<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub security_opt: Vec<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub configs: Vec<config::Config>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<Port>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<secret::Secret>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<volume::Volume>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub init: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub privileged: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub read_only: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub stdin_open: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub tty: bool,

    #[serde(flatten)]
    pub capabilities: capabilities::Capabilities,

    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    pub command: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    pub dns_search: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    pub entrypoint: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    pub env_file: string_or_list::StringList,
    #[serde(default)]
    #[serde(skip_serializing_if = "string_or_list::StringList::is_empty")]
    pub tmpfs: string_or_list::StringList,

    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "string_or_list::deserialize")]
    pub dns: Vec<std::net::IpAddr>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub environment: array_dict::ArrayDict,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub extra_hosts: array_dict::ArrayDict,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub labels: array_dict::ArrayDict,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub sysctls: array_dict::ArrayDict,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub deploy: Deployment,
    #[serde(default)]
    //#[serde(skip_serializing)]
    #[serde(skip_serializing_if = "is_default")]
    pub networks: network::Networks,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    pub _extras: Map<String, serde_yaml::Value>,
}

impl Service {
    pub fn armour_bridge_network(name: &str) -> String {
        format!("arm-{}", name)
    }
    fn network(
        name: &str,
        subnets: &mut ipnet::Ipv4Subnets,
    ) -> (
        network::Network,
        network::Networks,
        Option<std::net::Ipv4Addr>,
    ) {
        let armour_bridge_network = Service::armour_bridge_network(name);
        let mut network = network::Network::default();
        network.driver = Some(network::Driver::Bridge);
        network.driver_opts.insert(
            "com.docker.network.bridge.name".to_string(),
            armour_bridge_network.clone(),
        );
        network.internal = true;
        if let Some(subnet) = subnets.next() {
            let mut ipam = network::Ipam::default();
            ipam.config = vec![network::IpamConfig { subnet }];
            network.ipam = Some(ipam);
            let mut network_record = network::NetworkRecord::default();
            let ipv4_addr = subnet.hosts().nth(1);
            network_record.ipv4_address = ipv4_addr;
            let mut dict = Map::new();
            dict.insert(armour_bridge_network, network_record);
            (network, network::Networks::Dict(dict), ipv4_addr)
        } else {
            (
                network,
                network::Networks::Array(vec![armour_bridge_network]),
                None,
            )
        }
    }
    pub fn convert_for_armour(
        &mut self,
        name: &str,
        subnets: &mut ipnet::Ipv4Subnets,
    ) -> (ServiceInfo, network::Network, Option<std::net::Ipv4Addr>) {
        let info = ServiceInfo {
            armour_labels: self.armour.labels.clone(),
            // container_labels: self.labels.clone(),
            // network: armour_bridge_network.clone(),
            ipv4_address: None,
        };
        // create a new (internal) bridge network for the service
        let (network, networks, ipv4_addr) = Service::network(name, subnets);
        // wipe armour field
        self.armour = Armour::default();
        // use internal bridge network
        self.networks = networks;
        (info, network, ipv4_addr)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
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
//pub fn skip(m: &Armour>) -> bool {
//   true
//}
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

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Armour {
    pub labels: armour_lang::labels::Labels,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct PortRecord {
    target: u16,
    published: u16,
    protocol: String,
    mode: String,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Debug, Clone)]
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

#[derive(Serialize, Debug, PartialEq, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct Resource {
    #[serde(skip_serializing_if = "Option::is_none")]
    cpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory: Option<String>,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
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
    pub placement: Option<serde_yaml::Value>,

    #[serde(skip_serializing)]
    #[serde(flatten)]
    pub _extras: Map<String, serde_yaml::Value>,
}
