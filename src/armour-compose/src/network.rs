/*
 * Copyright (c) 2021 Arm Limited.
 *
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

 use armour_serde::{array_dict, deserialize_from_str, is_default};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap as Map;
use std::fmt;

#[derive(Serialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpamConfig {
    pub subnet: ipnet::Ipv4Net,
    //#[serde(skip_serializing)]
    //#[serde(flatten)]
    //#[serde(default)]
    //_extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Ipam {
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub driver: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<IpamConfig>,
    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    pub _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Network {
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver: Option<Driver>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub driver_opts: Map<String, String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipam: Option<Ipam>,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub external: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub internal: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub attachable: bool,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    pub labels: array_dict::ArrayDict,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    pub _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct NetworkRecord {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_address: Option<std::net::Ipv4Addr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ipv6_address: Option<std::net::Ipv6Addr>,
    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Debug, PartialEq, Clone)]
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
