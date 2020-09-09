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
    pub id:     u64,
    pub config: Config,
    pub state:  State,
    pub test_id: u64,
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
    #[serde(skip)]
    pub test_id: u64,
    pub target:  String,
    pub period:  u64,
    pub count:   u64,
    pub expiry:  u64,
}

#[derive(Debug, Deserialize)]
pub struct TraceConfig {
    #[serde(skip)]
    pub test_id: u64,
    pub target:  String,
    pub period:  u64,
    pub limit:   u64,
    pub expiry:  u64,
}

#[derive(Debug, Deserialize)]
pub struct FetchConfig {
    #[serde(skip)]
    pub test_id: u64,
    pub target:  String,
    pub period:  u64,
    pub expiry:  u64,
}

#[derive(Debug, Deserialize)]
pub struct KnockConfig {
    #[serde(skip)]
    pub test_id: u64,
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

        let id    = c.id;
        let state = c.state;
        let test_id = c.test_id;

        let config = if let Some(mut cfg) = c.ping {
            cfg.test_id = c.test_id;
            Config::Ping(cfg)
        } else if let Some(mut cfg) = c.trace {
            cfg.test_id = c.test_id;
            Config::Trace(cfg)
        } else if let Some(mut cfg) = c.fetch {
            cfg.test_id = c.test_id;
            Config::Fetch(cfg)
        } else if let Some(mut cfg) = c.knock {
            cfg.test_id = c.test_id;
            Config::Knock(cfg)
        } else {
            Config::Unknown
        };

        Ok(Task { id, config, state, test_id })
    }
}
