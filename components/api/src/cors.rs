// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::bail;
use serde::{de, ser};

use std::{fmt, str::FromStr};

/// CORS header specification.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AllowOrigin {
    /// Allows access from any host.
    Any,
    /// Allows access only from the specified hosts.
    Whitelist(Vec<String>),
}

impl ser::Serialize for AllowOrigin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            AllowOrigin::Any => "*".serialize(serializer),
            AllowOrigin::Whitelist(ref hosts) => {
                if hosts.len() == 1 {
                    hosts[0].serialize(serializer)
                } else {
                    hosts.serialize(serializer)
                }
            }
        }
    }
}

impl<'de> de::Deserialize<'de> for AllowOrigin {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = AllowOrigin;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a list of hosts or \"*\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<AllowOrigin, E>
            where
                E: de::Error,
            {
                match value {
                    "*" => Ok(AllowOrigin::Any),
                    _ => Ok(AllowOrigin::Whitelist(vec![value.to_string()])),
                }
            }

            fn visit_seq<A>(self, seq: A) -> Result<AllowOrigin, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let hosts =
                    de::Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(AllowOrigin::Whitelist(hosts))
            }
        }

        d.deserialize_any(Visitor)
    }
}

impl FromStr for AllowOrigin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "*" {
            return Ok(AllowOrigin::Any);
        }

        let v: Vec<_> = s
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if v.is_empty() {
            bail!("Invalid AllowOrigin::Whitelist value");
        }

        Ok(AllowOrigin::Whitelist(v))
    }
}

#[test]
fn allow_origin_from_str() {
    use pretty_assertions::assert_eq;

    fn check(text: &str, expected: AllowOrigin) {
        let from_str = AllowOrigin::from_str(text).unwrap();
        assert_eq!(from_str, expected);
    }

    check(r#"*"#, AllowOrigin::Any);
    check(
        r#"http://example.com"#,
        AllowOrigin::Whitelist(vec!["http://example.com".to_string()]),
    );
    check(
        r#"http://a.org, http://b.org"#,
        AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
    );
    check(
        r#"http://a.org, http://b.org, "#,
        AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
    );
    check(
        r#"http://a.org,http://b.org"#,
        AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
    );
}

#[test]
fn test_allow_origin_toml() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Config {
        allow_origin: AllowOrigin,
    }

    fn check(text: &str, allow_origin: AllowOrigin) {
        let config_toml = format!("allow_origin = {}\n", text);
        let config: Config = toml::from_str(&config_toml).unwrap();
        assert_eq!(config.allow_origin, allow_origin);
        assert_eq!(toml::to_string(&config).unwrap(), config_toml);
    }

    check(r#""*""#, AllowOrigin::Any);
    check(
        r#""http://example.com""#,
        AllowOrigin::Whitelist(vec!["http://example.com".to_string()]),
    );
    check(
        r#"["http://a.org", "http://b.org"]"#,
        AllowOrigin::Whitelist(vec!["http://a.org".to_string(), "http://b.org".to_string()]),
    );
}
