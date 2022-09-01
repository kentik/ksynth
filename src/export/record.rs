use std::collections::HashMap;
use std::convert::TryInto;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value};
use synapi::tasks::{Device, Kind};
use crate::net::tls::Identity;
use crate::stats::Summary;

#[derive(Clone, Debug)]
pub struct Target {
    pub company: u64,
    pub agent:   u64,
    pub device:  u64,
    pub columns: HashMap<String, Column>,
    pub email:   String,
    pub token:   String,
}

#[derive(Clone, Debug)]
pub struct Column {
    pub id:   u32,
    pub kind: Kind,
}

#[derive(Clone, Debug)]
pub enum Record {
    Fetch(Fetch),
    Knock(Knock),
    Opaque(Opaque),
    Ping(Ping),
    Query(Query),
    Shake(Shake),
    Trace(Trace),
    Error(Error),
    Timeout(Timeout),
}

#[derive(Clone, Debug)]
pub struct Fetch {
    pub task:    u64,
    pub test:    u64,
    pub target:  Arc<String>,
    pub addr:    IpAddr,
    pub server:  Identity,
    pub status:  u16,
    pub dns:     Duration,
    pub tcp:     Duration,
    pub tls:     Duration,
    pub rtt:     Duration,
    pub size:    usize,
}

#[derive(Clone, Debug)]
pub struct Knock {
    pub task:    u64,
    pub test:    u64,
    pub target:  Arc<String>,
    pub addr:    IpAddr,
    pub port:    u16,
    pub sent:    u32,
    pub lost:    u32,
    pub rtt:     Summary,
    pub result:  Vec<Duration>,
}

#[derive(Clone, Debug)]
pub struct Opaque {
    pub task:    u64,
    pub test:    u64,
    pub output:  Map<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Ping {
    pub task:    u64,
    pub test:    u64,
    pub target:  Arc<String>,
    pub addr:    IpAddr,
    pub sent:    u32,
    pub lost:    u32,
    pub rtt:     Summary,
    pub result:  Vec<Duration>,
}

#[derive(Clone, Debug)]
pub struct Query {
    pub task:    u64,
    pub test:    u64,
    pub code:    u16,
    pub record:  String,
    pub answers: String,
    pub time:    Duration,
}

#[derive(Clone, Debug)]
pub struct Shake {
    pub task:    u64,
    pub test:    u64,
    pub target:  Arc<String>,
    pub addr:    IpAddr,
    pub port:    u16,
    pub server:  Identity,
    pub time:    Duration,
}

#[derive(Clone, Debug)]
pub struct Trace {
    pub task:    u64,
    pub test:    u64,
    pub target:  Arc<String>,
    pub addr:    IpAddr,
    pub hops:    Vec<Hop>,
    pub route:   String,
    pub time:    Duration,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hop {
    pub hop:   usize,
    pub nodes: HashMap<IpAddr, Vec<u64>>,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub task:   u64,
    pub test:   u64,
    pub cause:  String,
}

#[derive(Clone, Debug)]
pub struct Timeout {
    pub task: u64,
    pub test: u64,
}

pub fn columns(device: Device) -> Result<HashMap<String, Column>> {
    device.columns.into_iter().map(|c| {
        let column = Column {
            id:   c.id.try_into()?,
            kind: c.kind,
        };
        Ok((c.name, column))
    }).collect()
}

impl From<Fetch> for Record  {
    fn from(fetch: Fetch) -> Self {
        Record::Fetch(fetch)
    }
}

impl From<Knock> for Record  {
    fn from(knock: Knock) -> Self {
        Record::Knock(knock)
    }
}

impl From<Opaque> for Record  {
    fn from(opaque: Opaque) -> Self {
        Record::Opaque(opaque)
    }
}

impl From<Ping> for Record  {
    fn from(ping: Ping) -> Self {
        Record::Ping(ping)
    }
}

impl From<Query> for Record  {
    fn from(query: Query) -> Self {
        Record::Query(query)
    }
}

impl From<Shake> for Record  {
    fn from(shake: Shake) -> Self {
        Record::Shake(shake)
    }
}

impl From<Trace> for Record  {
    fn from(trace: Trace) -> Self {
        Record::Trace(trace)
    }
}

impl From<Error> for Record  {
    fn from(error: Error) -> Self {
        Record::Error(error)
    }
}

impl From<Timeout> for Record  {
    fn from(timeout: Timeout) -> Self {
        Record::Timeout(timeout)
    }
}
