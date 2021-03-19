use std::collections::HashMap;
use serde::{Deserialize, de::Deserializer};
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

#[derive(Debug)]
pub struct Task {
    pub task:   u64,
    pub test:   u64,
    pub config: TaskConfig,
    pub family: Net,
    pub state:  State,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct FetchConfig {
    pub target:  String,
    pub period:  u64,
    pub expiry:  u64,
    #[serde(default)]
    pub method:  String,
    #[serde(default)]
    pub body:    Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct KnockConfig {
    pub target:  String,
    pub period:  u64,
    pub count:   u64,
    pub expiry:  u64,
    pub port:    u16,
}

#[derive(Debug, Deserialize)]
pub struct PingConfig {
    pub target:  String,
    pub period:  u64,
    pub count:   u64,
    pub expiry:  u64,
}

#[derive(Debug, Deserialize)]
pub struct QueryConfig {
    pub target:  String,
    pub period:  u64,
    pub expiry:  u64,
    #[serde(rename = "resolver")]
    pub server:  String,
    pub port:    u16,
    #[serde(rename = "type")]
    pub record:  String,
}

#[derive(Debug, Deserialize)]
pub struct ShakeConfig {
    pub target:   String,
    pub port:     u16,
    pub period:   u64,
    pub expiry:   u64,
}

#[derive(Debug, Deserialize)]
pub struct TraceConfig {
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub port:     u16,
    pub target:   String,
    pub period:   u64,
    #[serde(default = "default_trace_count")]
    pub count:    u64,
    pub limit:    u64,
    pub expiry:   u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum State {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Deserialize)]
pub struct Kentik {
    pub email: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct Device {
    #[serde(deserialize_with = "id")]
    pub id:      u64,
    #[serde(rename = "customs")]
    pub columns: Vec<Column>,
}

#[derive(Debug, Deserialize)]
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

fn default_trace_count() -> u64 {
    3
}
