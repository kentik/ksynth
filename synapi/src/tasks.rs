use serde::{Deserialize, de::{Deserializer, Error, Unexpected}};
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
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct PingConfig {
    pub target: String,
    pub period: u64,
    pub count:  u64,
    pub expiry: u64,
}

#[derive(Debug, Deserialize)]
pub struct TraceConfig {
    pub target: String,
    pub period: u64,
    pub limit:  u64,
    pub expiry: u64,
}

#[derive(Debug, Deserialize)]
pub struct FetchConfig {
    pub target: String,
    pub period: u64,
    pub expiry: u64,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum Kind {
    U32,
    U64,
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
            pub state: State,
            #[serde(deserialize_with = "id", rename = "test_id")]
            pub test_id: u64,
        }

        let c = TaskContainer::deserialize(de)?;

        let id    = c.id;
        let state = c.state;
        let test_id = c.test_id;

        let config = if let Some(cfg) = c.ping {
            Config::Ping(cfg)
        } else if let Some(cfg) = c.trace {
            Config::Trace(cfg)
        } else if let Some(cfg) = c.fetch {
            Config::Fetch(cfg)
        } else {
            Config::Unknown
        };

        Ok(Task { id, config, state, test_id })
    }
}

impl<'d> Deserialize<'d> for State {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        match u64::deserialize(de)? {
            0 => Ok(State::Created),
            1 => Ok(State::Updated),
            2 => Ok(State::Deleted),
            n => Err(Error::invalid_value(Unexpected::Unsigned(n), &"0..2")),
        }
    }
}

impl<'d> Deserialize<'d> for Kind {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        match u64::deserialize(de)? {
            0 => Ok(Kind::U32),
            1 => Ok(Kind::U64),
            2 => Ok(Kind::String),
            3 => Ok(Kind::Addr),
            n => Err(Error::invalid_value(Unexpected::Unsigned(n), &"0..3")),
        }
    }
}
