use super::serde_utils::{array_dict, is_default, string_or_list, string_or_struct};
use super::{build, capabilities, config, network, ports, secret, volume};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap as Map;

#[derive(Serialize, Deserialize, Debug)]
pub struct Service {
    // TODO:
    //
    // credential_check
    // deploy
    // healthcheck
    // logging
    // ulimits
    //
    #[serde(default)]
    #[serde(deserialize_with = "string_or_struct::deserialize")]
    #[serde(skip_serializing_if = "is_default")]
    build: build::Build,

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
    ports: Vec<ports::Port>,
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
    networks: network::Networks,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}
