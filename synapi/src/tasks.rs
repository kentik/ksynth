use std::cmp::max;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::time::Duration;
use serde::{Deserialize, de::{Deserializer, Error, Visitor}};
use crate::serde::id;
use super::agent::Net;

#[derive(Debug, Deserialize)]
pub struct Tasks {
    #[serde(deserialize_with = "id")]
    pub timestamp: u64,
    pub groups:    Vec<Group>,
}

#[derive(Debug, Deserialize)]
pub struct Group {
    #[serde(rename = "company_id", deserialize_with = "id")]
    pub company: u64,
    pub kentik:  Kentik,
    pub device:  Device,
    pub tasks:   Vec<Task>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub task:   u64,
    pub test:   u64,
    pub config: TaskConfig,
    pub family: Net,
    pub state:  State,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskConfig {
    Fetch(FetchConfig),
    Knock(KnockConfig),
    Ping(PingConfig),
    Query(QueryConfig),
    Shake(ShakeConfig),
    Trace(TraceConfig),
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FetchConfig {
    pub target:   String,
    pub period:   Period,
    pub expiry:   Expiry,
    #[serde(default)]
    pub method:   String,
    #[serde(default)]
    pub body:     Option<String>,
    #[serde(default)]
    pub headers:  Option<HashMap<String, String>>,
    #[serde(rename = "ignore_tls_errors", default)]
    pub insecure: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KnockConfig {
    pub target:  String,
    pub period:  Period,
    pub count:   Count,
    #[serde(default)]
    pub delay:   Delay,
    pub expiry:  Expiry,
    pub port:    u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PingConfig {
    pub target:  String,
    pub period:  Period,
    pub count:   Count,
    #[serde(default)]
    pub delay:   Delay,
    pub expiry:  Expiry,
}

#[derive(Clone, Debug, Deserialize)]
pub struct QueryConfig {
    pub target:  String,
    pub period:  Period,
    pub expiry:  Expiry,
    #[serde(rename = "resolver")]
    pub server:  String,
    pub port:    u16,
    #[serde(rename = "type")]
    pub record:  String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ShakeConfig {
    pub target:   String,
    pub port:     u16,
    pub period:   Period,
    pub expiry:   Expiry,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TraceConfig {
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub port:     u16,
    pub target:   String,
    pub period:   Period,
    #[serde(default = "default_trace_count")]
    pub count:    Count,
    pub limit:    Limit,
    #[serde(default)]
    pub delay:    Delay,
    pub expiry:   Expiry,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum State {
    Created,
    Updated,
    Deleted,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Kentik {
    pub email: String,
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Device {
    #[serde(deserialize_with = "id")]
    pub id:      u64,
    #[serde(rename = "customs")]
    pub columns: Vec<Column>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Column {
    #[serde(deserialize_with = "id")]
    pub id:   u64,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Kind,
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Kind {
    UInt32,
    UInt64,
    String,
    Addr,
}

#[derive(Copy, Clone, Debug)]
pub struct Count(usize);

#[derive(Copy, Clone, Debug)]
pub struct Limit(usize);

#[derive(Copy, Clone, Debug)]
pub struct Delay(Duration);

#[derive(Copy, Clone, Debug)]
pub struct Expiry(Duration);

#[derive(Copy, Clone, Debug)]
pub struct Period(Duration);

impl<'d> Deserialize<'d> for Task {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        #[derive(Debug, Deserialize)]
        struct TaskContainer {
            #[serde(deserialize_with = "id")]
            pub id:    u64,
            #[serde(rename = "http")]
            pub fetch: Option<FetchConfig>,
            pub knock: Option<KnockConfig>,
            pub ping:  Option<PingConfig>,
            #[serde(rename = "dns")]
            pub query: Option<QueryConfig>,
            pub shake: Option<ShakeConfig>,
            #[serde(rename = "traceroute")]
            pub trace: Option<TraceConfig>,
            pub state: State,
            #[serde(deserialize_with = "id")]
            pub test_id: u64,
            pub family:  Net,
        }

        let c = TaskContainer::deserialize(de)?;

        let task   = c.id;
        let test   = c.test_id;
        let family = c.family;
        let state  = c.state;

        let config = if let Some(cfg) = c.fetch {
            TaskConfig::Fetch(cfg)
        } else if let Some(cfg) = c.knock {
            TaskConfig::Knock(cfg)
        } else if let Some(cfg) = c.ping {
            TaskConfig::Ping(cfg)
        } else if let Some(cfg) = c.query {
            TaskConfig::Query(cfg)
        } else if let Some(cfg) = c.shake {
            TaskConfig::Shake(cfg)
        } else if let Some(cfg) = c.trace {
            TaskConfig::Trace(cfg)
        } else {
            TaskConfig::Unknown
        };

        Ok(Task { task, test, config, family, state })
    }
}

struct U64Visitor;

impl<'de> Visitor<'de> for U64Visitor {
    type Value = u64;

    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str("an unsigned 64-bit integer")
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(value)
    }
}

impl<'de> Deserialize<'de> for Count {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let n = de.deserialize_u64(U64Visitor)?;
        match usize::try_from(n) {
            Ok(0)  => Ok(Self(1)),
            Ok(n)  => Ok(Self(n)),
            Err(_) => Err(D::Error::custom(format!("count out of range: {}", n))),
        }
    }
}

impl<'de> Deserialize<'de> for Limit {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let n = de.deserialize_u64(U64Visitor)?;
        match usize::try_from(n) {
            Ok(0)  => Ok(Self(1)),
            Ok(n)  => Ok(Self(n)),
            Err(_) => Err(D::Error::custom(format!("limit out of range: {}", n))),
        }
    }
}

impl<'de> Deserialize<'de> for Delay {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let millis = de.deserialize_u64(U64Visitor)?;
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl<'de> Deserialize<'de> for Expiry {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let millis = max(de.deserialize_u64(U64Visitor)?, 1);
        Ok(Self(Duration::from_millis(millis)))
    }
}

impl<'de> Deserialize<'de> for Period {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let secs = max(de.deserialize_u64(U64Visitor)?, 1);
        Ok(Self(Duration::from_secs(secs)))
    }
}

impl From<Count> for usize {
    fn from(count: Count) -> Self {
        count.0
    }
}

impl From<usize> for Count {
    fn from(count: usize) -> Self {
        Self(count)
    }
}

impl From<Limit> for usize {
    fn from(limit: Limit) -> Self {
        limit.0
    }
}

impl From<usize> for Limit {
    fn from(count: usize) -> Self {
        Self(count)
    }
}

impl From<Delay> for Duration  {
    fn from(delay: Delay) -> Self {
        delay.0
    }
}

impl From<Duration> for Delay {
    fn from(delay: Duration) -> Self {
        Self(delay)
    }
}

impl From<Expiry> for Duration  {
    fn from(expiry: Expiry) -> Self {
        expiry.0
    }
}

impl From<Duration> for Expiry {
    fn from(expiry: Duration) -> Self {
        Self(expiry)
    }
}

impl From<Period> for Duration  {
    fn from(period: Period) -> Self {
        period.0
    }
}

impl From<Duration> for Period {
    fn from(period: Duration) -> Self {
        Self(period)
    }
}

impl Default for Delay  {
    fn default() -> Self {
       Self(Duration::from_millis(0))
    }
}

fn default_trace_count() -> Count {
    Count(3)
}
