use std::net::IpAddr;
use anyhow::Result;
use serde::Serialize;
use crate::export::{Record, record};

#[derive(Debug, Serialize)]
#[serde(tag = "eventType", rename_all = "lowercase")]
pub enum Event<'a> {
    Fetch(Fetch<'a>),
    Knock(Knock<'a>),
    Ping(Ping<'a>),
    Query(Query<'a>),
    Shake(Shake<'a>),
    Trace(Trace<'a>),
    Error(Error<'a>),
    Timeout,
}

#[derive(Debug, Serialize)]
pub struct Fetch<'a> {
    agent:  &'a str,
    target: &'a str,
    addr:   &'a IpAddr,
    status: u16,
    dns:    u128,
    tcp:    u128,
    tls:    u128,
    rtt:    u128,
    size:   usize,
}

#[derive(Debug, Serialize)]
pub struct Knock<'a> {
    agent:  &'a str,
    target: &'a str,
    addr:   &'a IpAddr,
    port:   u16,
    sent:   u32,
    lost:   u32,
}

#[derive(Debug, Serialize)]
pub struct Ping<'a> {
    agent:  &'a str,
    target: &'a str,
    addr:   &'a IpAddr,
    sent:   u32,
    lost:   u32,
}

#[derive(Debug, Serialize)]
pub struct Query<'a> {
    agent:   &'a str,
    record:  &'a str,
    code:    u16,
    answers: &'a str,
    time:    u128,
}

#[derive(Debug, Serialize)]
pub struct Shake<'a> {
    agent:  &'a str,
    target: &'a str,
    addr:   &'a IpAddr,
    port:   u16,
    time:   u128,
}

#[derive(Debug, Serialize)]
pub struct Trace<'a> {
    agent:  &'a str,
    target: &'a str,
    addr:   &'a IpAddr,
    hops:   usize,
    time:   u128,
}

#[derive(Debug, Serialize)]
pub struct Error<'a> {
    agent: &'a str,
    cause: &'a str,
}

pub fn encode(agent: &str, rs: &[Record], buf: &mut Vec<u8>) -> Result<()> {
    Ok(serde_json::to_writer(buf, &rs.iter().map(|r| {
        Ok(match r {
            Record::Fetch(data)   => fetch(data, agent)?,
            Record::Knock(data)   => knock(data, agent)?,
            Record::Ping(data)    => ping(data, agent)?,
            Record::Query(data)   => query(data, agent)?,
            Record::Shake(data)   => shake(data, agent)?,
            Record::Trace(data)   => trace(data, agent)?,
            Record::Error(data)   => error(data, agent)?,
            Record::Timeout(_)    => Event::Timeout,
        })
    }).collect::<Result<Vec<_>>>()?)?)
}

fn fetch<'a>(data: &'a record::Fetch, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Fetch(Fetch {
        agent:  agent,
        target: &data.target,
        addr:   &data.addr,
        status: data.status,
        dns:    data.dns.as_micros(),
        tcp:    data.tcp.as_micros(),
        tls:    data.tls.as_micros(),
        rtt:    data.rtt.as_micros(),
        size:   data.size,
    }))
}

fn knock<'a>(data: &'a record::Knock, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Knock(Knock {
        agent:  agent,
        target: &data.target,
        addr:   &data.addr,
        port:   data.port,
        sent:   data.sent,
        lost:   data.lost,
    }))
}

fn ping<'a>(data: &'a record::Ping, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Ping(Ping {
        agent:  agent,
        target: &data.target,
        addr:   &data.addr,
        sent:   data.sent,
        lost:   data.lost,
    }))
}

fn query<'a>(data: &'a record::Query, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Query(Query {
        agent:   agent,
        record:  &data.record,
        code:    data.code,
        answers: &data.answers,
        time:    data.time.as_micros(),
    }))
}

fn shake<'a>(data: &'a record::Shake, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Shake(Shake {
        agent:  agent,
        target: &data.target,
        addr:   &data.addr,
        port:   data.port,
        time:   data.time.as_micros(),
    }))
}

fn trace<'a>(data: &'a record::Trace, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Trace(Trace {
        agent:  agent,
        target: &data.target,
        addr:   &data.addr,
        hops:   data.hops.len(),
        time:   data.time.as_micros(),
    }))
}

fn error<'a>(data: &'a record::Error, agent: &'a str) -> Result<Event<'a>> {
    Ok(Event::Error(Error {
        agent:  agent,
        cause:  &data.cause,
    }))
}
