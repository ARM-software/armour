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

pub fn is_default<T>(t: &T) -> bool
where
    T: Default + PartialEq,
{
    *t == T::default()
}

// provide deserialization support for types that implement the trait `std::str::FromStr`.
#[macro_export]
macro_rules! deserialize_from_str {
    ($ty:ident) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<$ty, D::Error>
            where
                D: Deserializer<'de>,
            {
                armour_serde::from_str::deserialize(deserializer)
            }
        }
    };
}

pub mod from_str {
    use serde::de::{self, Visitor};
    use serde::{Deserialize, Deserializer};
    use std::fmt;
    use std::marker::PhantomData;
    use std::str::FromStr;

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: Deserialize<'de> + FromStr,
        D: Deserializer<'de>,
        T::Err: std::fmt::Display,
    {
        struct FromString<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for FromString<T>
        where
            T: Deserialize<'de> + FromStr,
            T::Err: fmt::Display,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("from string")
            }

            fn visit_str<E>(self, value: &str) -> Result<T, E>
            where
                E: de::Error,
            {
                Ok(FromStr::from_str(value).map_err(|e| E::custom(format!("{}", e)))?)
            }
        }

        deserializer.deserialize_any(FromString(PhantomData))
    }
}

pub mod string_or_list {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::fmt;
    use std::marker::PhantomData;
    use std::str::FromStr;

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        T: Deserialize<'de> + FromStr,
        D: Deserializer<'de>,
        T::Err: fmt::Display,
    {
        struct FromString<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for FromString<T>
        where
            T: Deserialize<'de> + FromStr,
            T::Err: fmt::Display,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("from string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Vec<T>, E>
            where
                E: de::Error,
            {
                Ok(vec![
                    FromStr::from_str(value).map_err(|e| E::custom(format!("{}", e)))?
                ])
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Vec<T>, A::Error>
            where
                A: SeqAccess<'de>,
                A::Error: de::Error,
            {
                let mut v = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(value) = seq.next_element()? {
                    v.push(value);
                }
                Ok(v)
            }
        }

        deserializer.deserialize_any(FromString(PhantomData))
    }

    #[derive(Default, Debug, Deserialize, Clone)]
    #[serde(transparent)]
    pub struct StringList(#[serde(deserialize_with = "deserialize")] Vec<String>);

    impl StringList {
        pub fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
    }

    impl Serialize for StringList {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let n = self.0.len();

            if n == 1 {
                serializer.serialize_str(self.0.get(0).unwrap())
            } else {
                let mut seq = serializer.serialize_seq(Some(n))?;
                for e in self.0.iter() {
                    seq.serialize_element(&e)?;
                }
                seq.end()
            }
        }
    }
}

pub mod string_or_struct {
    use serde::de::{self, MapAccess, Visitor};
    use serde::{Deserialize, Deserializer};
    use std::fmt;
    use std::marker::PhantomData;
    use std::str::FromStr;

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: Deserialize<'de> + FromStr,
        D: Deserializer<'de>,
        T::Err: std::fmt::Display,
    {
        struct StringOrStruct<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for StringOrStruct<T>
        where
            T: Deserialize<'de> + FromStr,
            T::Err: fmt::Display,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or map")
            }

            fn visit_str<E>(self, value: &str) -> Result<T, E>
            where
                E: de::Error,
            {
                Ok(FromStr::from_str(value).map_err(|e| E::custom(format!("{}", e)))?)
            }

            fn visit_map<M>(self, map: M) -> Result<T, M::Error>
            where
                M: MapAccess<'de>,
            {
                Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(StringOrStruct(PhantomData))
    }
}

pub mod array_dict {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap as Map;

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    #[serde(untagged)]
    pub enum ArrayDict {
        Array(Vec<String>),
        Dict(Map<String, String>),
    }

    impl Default for ArrayDict {
        fn default() -> ArrayDict {
            ArrayDict::Array(Vec::new())
        }
    }
}
