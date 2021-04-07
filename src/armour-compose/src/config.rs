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

use super::external;
use armour_serde::{array_dict, is_default};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap as Map;
use std::fmt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigConfig {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigRecord {
    #[serde(default)]
    #[serde(skip_serializing_if = "is_default")]
    source: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    uid: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    gid: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<u16>,
    #[serde(skip_serializing)]
    #[serde(flatten)]
    _extras: Map<String, serde_yaml::Value>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Config {
    Raw(String),
    Struct(ConfigRecord),
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ConfigVisitor;

        impl<'de> Visitor<'de> for ConfigVisitor {
            type Value = Config;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or map")
            }

            fn visit_str<E>(self, value: &str) -> Result<Config, E>
            where
                E: de::Error,
            {
                Ok(Config::Raw(value.to_owned()))
            }

            fn visit_map<V>(self, map: V) -> Result<Config, V::Error>
            where
                V: MapAccess<'de>,
                V::Error: de::Error,
            {
                Ok(Config::Struct(
                    ConfigRecord::deserialize(de::value::MapAccessDeserializer::new(map))
                        .map_err(de::Error::custom)?,
                ))
            }
        }

        deserializer.deserialize_any(ConfigVisitor)
    }
}
