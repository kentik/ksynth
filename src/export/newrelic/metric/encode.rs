use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::Result;
use serde::Serialize;
use crate::export::{Record, record::*};
use super::metric::{Attribute, Attributes, Metric};

#[derive(Debug, Default, Serialize)]
pub struct Payload<'a> {
    pub metrics: &'a [Metric<'a>],
    pub common:  Common<'a>,
}

#[derive(Debug, Default, Serialize)]
pub struct Common<'a> {
    pub attributes: Attributes<'a>,
}

pub fn encode(agent: &str, rs: &[Record], buf: &mut Vec<u8>) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?;

    buf.push(b'[');
    for record in rs {
        match record {
            Record::Fetch(data)   => fetch(data, agent, timestamp, buf)?,
            Record::Knock(data)   => knock(data, agent, timestamp, buf)?,
            Record::Opaque(..)    => continue,
            Record::Ping(data)    => ping(data, agent, timestamp, buf)?,
            Record::Query(data)   => query(data, agent, timestamp, buf)?,
            Record::Shake(data)   => shake(data, agent, timestamp, buf)?,
            Record::Trace(data)   => trace(data, agent, timestamp, buf)?,
            Record::Error(_)      => continue,
            Record::Timeout(_)    => continue,
        }
        buf.push(b',');
    }
    buf.pop();
    buf.push(b']');

    Ok(())
}

fn fetch(data: &Fetch, agent: &str, ts: Duration, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let code = data.status as f64;
    let rtt  = as_micros(data.rtt);
    let dns  = as_micros(data.rtt);
    let tcp  = as_micros(data.tcp);
    let tls  = as_micros(data.tls);
    let size = data.size as f64;

    let common = &[
        Attribute::String("agent",  agent),
        Attribute::String("task",   "fetch"),
        Attribute::String("target", &data.target),
        Attribute::String("addr",   &addr),
    ];

    let code = Metric::gauge("ksynth.fetch.code", code, ts);
    let size = Metric::gauge("ksynth.fetch.size", size, ts);
    let rtt  = Metric::gauge("ksynth.fetch.rtt",  rtt, ts);
    let dns  = Metric::gauge("ksynth.fetch.dns",  dns, ts);
    let tcp  = Metric::gauge("ksynth.fetch.tcp",  tcp, ts);
    let tls  = Metric::gauge("ksynth.fetch.tls",  tls, ts);

    let attributes = Attributes(common);
    serde_json::to_writer(buf, &Payload {
        metrics: &[code, size, rtt, dns, tcp, tls],
        common:  Common { attributes },
    })?;

    Ok(())
}

fn knock(data: &Knock, agent: &str, ts: Duration, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let lost = f64::from(data.lost);
    let sent = f64::from(data.sent);
    let loss = lost / sent;

    let common = &[
        Attribute::String("agent",  agent),
        Attribute::String("task",   "fetch"),
        Attribute::String("target", &data.target),
        Attribute::String("addr",   &addr),
    ];

    let mut metrics = Vec::new();
    for rtt in &data.result {
        let time = as_micros(*rtt);
        metrics.push(Metric::gauge("ksynth.knock.lost", lost, ts));
        metrics.push(Metric::gauge("ksynth.knock.sent", sent, ts));
        metrics.push(Metric::gauge("ksynth.knock.loss", loss, ts));
        metrics.push(Metric::gauge("ksynth.knock.rtt",  time, ts));
    }

    let attributes = Attributes(common);
    serde_json::to_writer(buf, &Payload {
        metrics: &metrics,
        common:  Common { attributes },
    })?;

    Ok(())
}

fn ping(data: &Ping, agent: &str, ts: Duration, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let lost = f64::from(data.lost);
    let sent = f64::from(data.sent);
    let loss = lost / sent;

    let common = &[
        Attribute::String("agent",  agent),
        Attribute::String("task",   "fetch"),
        Attribute::String("target", &data.target),
        Attribute::String("addr",   &addr),
    ];

    let mut metrics = Vec::new();
    for rtt in &data.result {
        let time = as_micros(*rtt);
        metrics.push(Metric::gauge("ksynth.ping.lost", lost, ts));
        metrics.push(Metric::gauge("ksynth.ping.sent", sent, ts));
        metrics.push(Metric::gauge("ksynth.ping.loss", loss, ts));
        metrics.push(Metric::gauge("ksynth.ping.rtt",  time, ts));
    }

    let attributes = Attributes(common);
    serde_json::to_writer(buf, &Payload {
        metrics: &metrics,
        common:  Common { attributes },
    })?;

    Ok(())
}

fn query(data: &Query, agent: &str, ts: Duration, buf: &mut Vec<u8>) -> Result<()> {
    let code = f64::from(data.code);
    let time = as_micros(data.time);

    let common = &[
        Attribute::String("agent", agent),
        Attribute::String("task",  "fetch"),
    ];

    let code = Metric::gauge("ksynth.query.code", code, ts);
    let rtt  = Metric::gauge("ksynth.query.rtt",  time, ts);

    let attributes = Attributes(common);
    serde_json::to_writer(buf, &Payload {
        metrics: &[code, rtt],
        common:  Common { attributes },
    })?;

    Ok(())
}

fn shake(data: &Shake, agent: &str, ts: Duration, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let time = as_micros(data.time);
    let port = u64::from(data.port);

    let common = &[
        Attribute::String("agent",  agent),
        Attribute::String("task",   "fetch"),
        Attribute::String("target", &data.target),
        Attribute::String("addr",   &addr),
        Attribute::Number("port",   port),
    ];

    let rtt = Metric::gauge("ksynth.shake.rtt",  time, ts);

    let attributes = Attributes(common);
    serde_json::to_writer(buf, &Payload {
        metrics: &[rtt],
        common:  Common { attributes },
    })?;

    Ok(())
}

fn trace(data: &Trace, agent: &str, ts: Duration, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let time = as_micros(data.time);
    let hops = data.hops.iter().map(|hop| hop.hop).max();
    let hops = hops.unwrap_or_default() as f64;

    let common = &[
        Attribute::String("agent",  agent),
        Attribute::String("task",   "fetch"),
        Attribute::String("target", &data.target),
        Attribute::String("addr",   &addr),
    ];

    let hops = Metric::gauge("ksynth.trace.hops", hops, ts);
    let rtt  = Metric::gauge("ksynth.trace.rtt",  time, ts);

    let attributes = Attributes(common);
    serde_json::to_writer(buf, &Payload {
        metrics: &[hops, rtt],
        common:  Common { attributes },
    })?;

    Ok(())
}

fn as_micros(d: Duration) -> f64 {
    d.as_micros() as f64
}
