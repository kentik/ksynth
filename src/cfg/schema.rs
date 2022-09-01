use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;
use anyhow::Result;
use serde::{Deserialize, de::{Deserializer, Error, Visitor}};
use serde_json::{Map, Value};
use crate::net::Network;

#[derive(Debug, Deserialize)]
pub struct Tasks {
    pub tasks: Vec<Task>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Task {
    #[serde(flatten)]
    pub config:  Config,
    pub network: Network,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Config {
    Fetch(Fetch),
    Knock(Knock),
    Opaque(Opaque),
    Ping(Ping),
    Query(Query),
    Shake(Shake),
    Trace(Trace),
}

#[derive(Clone, Debug, Deserialize)]
pub struct Fetch {
    pub target:   String,
    pub method:   String,
    pub body:     Option<String>,
    pub headers:  Option<HashMap<String, String>>,
    pub insecure: bool,
    pub period:   Time,
    pub expiry:   Time,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Knock {
    pub target: String,
    pub port:   u16,
    pub count:  Count,
    pub period: Time,
    pub delay:  Time,
    pub expiry: Time,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Opaque {
    pub method: String,
    pub config: Map<String, Value>,
    pub period: Time,
    pub expiry: Time,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ping {
    pub target: String,
    pub count:  Count,
    pub period: Time,
    pub delay:  Time,
    pub expiry: Time,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Query {
    pub target: String,
    pub server: String,
    pub port:   u16,
    pub record: String,
    pub period: Time,
    pub expiry: Time,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Shake {
    pub target: String,
    pub port:   u16,
    pub period: Time,
    pub expiry: Time,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Trace {
    pub target:   String,
    pub protocol: String,
    pub port:     u16,
    pub count:    Count,
    pub limit:    Count,
    pub period:   Time,
    pub delay:    Time,
    pub expiry:   Time,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Time(pub Duration);

#[derive(Clone, Debug, PartialEq)]
pub struct Count(pub u64);

impl Time {
    pub fn as_secs(&self) -> u64 {
        self.0.as_secs()
    }

    pub fn as_micros(&self) -> Result<u64> {
        Ok(u64::try_from(self.0.as_micros())?)
    }

    pub fn as_millis(&self) -> Result<u64> {
        Ok(u64::try_from(self.0.as_millis())?)
    }
}

impl<'d> Deserialize<'d> for Count {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        Ok(Count(de.deserialize_u64(U64Visitor)?))
    }
}

impl<'d> Deserialize<'d> for Time {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(TimeVisitor)
    }
}

struct U64Visitor;

impl<'d> Visitor<'d> for U64Visitor {
    type Value = u64;

    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("an unsigned 64-bit integer")
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(value)
    }
}

struct TimeVisitor;

impl<'d> Visitor<'d> for TimeVisitor {
    type Value = Time;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a unit of time")
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Time(Duration::from_secs(value)))
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
        let (count, unit) = value.find(char::is_alphabetic).map(|n| {
            value.split_at(n)
        }).unwrap_or((value, "s"));

        let count = match u64::from_str(count) {
            Ok(count) => count,
            Err(_)    => return Err(E::custom("invalid count")),
        };

        Ok(Time(match unit {
            "s"  => Duration::from_secs(count),
            "ms" => Duration::from_millis(count),
            "us" => Duration::from_micros(count),
            "ns" => Duration::from_nanos(count),
            _    => return Err(E::custom("invalid unit")),
        }))
    }
}

#[test]
fn time() -> Result<()> {
    let expect = Time(Duration::from_secs(10));
    let actual = serde_yaml::from_str("10")?;
    assert_eq!(expect, actual);

    let expect = Time(Duration::from_secs(10));
    let actual = serde_yaml::from_str(r#""10""#)?;
    assert_eq!(expect, actual);

    let expect = Time(Duration::from_secs(10));
    let actual = serde_yaml::from_str("10s")?;
    assert_eq!(expect, actual);

    let expect = Time(Duration::from_millis(10));
    let actual = serde_yaml::from_str("10ms")?;
    assert_eq!(expect, actual);

    let expect = Time(Duration::from_micros(10));
    let actual = serde_yaml::from_str("10us")?;
    assert_eq!(expect, actual);

    let expect = Time(Duration::from_nanos(10));
    let actual = serde_yaml::from_str("10ns")?;
    assert_eq!(expect, actual);

    Ok(())
}
