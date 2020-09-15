use serde::{Deserialize, de::Deserializer};
use crate::serde::id;

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
    pub config: Config,
    pub state:  State,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Config {
    Ping(PingConfig),
    Trace(TraceConfig),
    Fetch(FetchConfig),
    Knock(KnockConfig),
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct PingConfig {
    pub target:  String,
    pub period:  u64,
    pub count:   u64,
    pub expiry:  u64,
}

#[derive(Debug, Deserialize)]
pub struct TraceConfig {
    pub target:  String,
    pub period:  u64,
    pub limit:   u64,
    pub expiry:  u64,
}

#[derive(Debug, Deserialize)]
pub struct FetchConfig {
    pub target:  String,
    pub period:  u64,
    pub expiry:  u64,
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

#[derive(Debug, Deserialize)]
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
            pub ping:  Option<PingConfig>,
            #[serde(rename = "traceroute")]
            pub trace: Option<TraceConfig>,
            #[serde(rename = "http")]
            pub fetch: Option<FetchConfig>,
            pub knock: Option<KnockConfig>,
            pub state: State,
            #[serde(deserialize_with = "id")]
            pub test_id: u64,
        }

        let c = TaskContainer::deserialize(de)?;

        let task  = c.id;
        let test  = c.test_id;
        let state = c.state;

        let config = if let Some(cfg) = c.ping {
            Config::Ping(cfg)
        } else if let Some(cfg) = c.trace {
            Config::Trace(cfg)
        } else if let Some(cfg) = c.fetch {
            Config::Fetch(cfg)
        } else if let Some(cfg) = c.knock {
            Config::Knock(cfg)
        } else {
            Config::Unknown
        };

        Ok(Task { task, test, config, state })
    }
}
