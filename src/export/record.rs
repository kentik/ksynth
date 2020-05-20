use std::collections::HashMap;
use std::net::IpAddr;
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
    Ping(Ping),
    Trace(Trace),
    Error(Error),
    Timeout(Timeout),
}

#[derive(Debug)]
pub struct Fetch {
    pub id:     u64,
    pub addr:   IpAddr,
    pub status: u16,
    pub rtt:    Duration,
    pub size:   usize,
}

#[derive(Debug)]
pub struct Ping {
    pub id:   u64,
    pub addr: IpAddr,
    pub sent: u32,
    pub lost: u32,
    pub rtt:  Summary,
}

#[derive(Debug)]
pub struct Trace {
    pub id:    u64,
    pub addr:  IpAddr,
    pub route: String,
    pub time:  Duration,
}

#[derive(Debug, Serialize)]
pub struct Hop {
    pub hop:   usize,
    pub nodes: HashMap<IpAddr, Vec<u64>>,
}

#[derive(Debug)]
pub struct Error {
    pub id:    u64,
    pub cause: String,
}

#[derive(Debug)]
pub struct Timeout {
    pub id: u64,
}

impl From<Fetch> for Record  {
    fn from(fetch: Fetch) -> Self {
        Record::Fetch(fetch)
    }
}

impl From<Ping> for Record  {
    fn from(ping: Ping) -> Self {
        Record::Ping(ping)
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
