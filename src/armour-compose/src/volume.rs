use super::external;
use armour_serde::{array_dict, is_default};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap as Map;
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeConfig {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    driver: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Map::is_empty")]
    driver_opts: Map<String, String>,

    #[serde(default)]
    external: external::External,

    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    labels: array_dict::ArrayDict,

    // capture everything else (future proofing)
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct Bind {
    popogation: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct VolumeOptions {
    nocopy: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default, Clone)]
pub struct Tmpfs {
    size: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeRecord {
    #[serde(default)]
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "is_default")]
    typ: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    source: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    target: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    read_only: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    consistency: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    bind: Bind,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    volume: VolumeOptions,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    tmpfs: Tmpfs,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Volume {
    Raw(String),
    Struct(VolumeRecord),
}

impl<'de> Deserialize<'de> for Volume {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VolumeVisitor;

        impl<'de> Visitor<'de> for VolumeVisitor {
            type Value = Volume;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or map")
            }

            fn visit_str<E>(self, value: &str) -> Result<Volume, E>
            where
                E: de::Error,
            {
                Ok(Volume::Raw(value.to_owned()))
            }

            fn visit_map<V>(self, map: V) -> Result<Volume, V::Error>
            where
                V: MapAccess<'de>,
                V::Error: de::Error,
            {
                Ok(Volume::Struct(
                    VolumeRecord::deserialize(de::value::MapAccessDeserializer::new(map))
                        .map_err(de::Error::custom)?,
                ))
            }
        }

        deserializer.deserialize_any(VolumeVisitor)
    }
}
