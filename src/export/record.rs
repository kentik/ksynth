use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use serde::Serialize;
use synapi::tasks::Device;
use crate::stats::Summary;

#[derive(Debug)]
pub struct Target {
    pub company: u64,
    pub agent:   u64,
    pub device:  Device,
    pub email:   String,
    pub token:   String,
}

#[derive(Debug)]
pub enum Record {
    Fetch(Fetch),
    Knock(Knock),
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
