use std::convert::TryFrom;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::Result;
use crate::export::{Record, record::*};
use super::wire::{Field, Point, Tag};

pub fn encode(agent: &str, rs: &[Record], buf: &mut Vec<u8>) -> Result<()> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

    for record in rs {
        match record {
            Record::Fetch(data)   => fetch(data, agent, timestamp, buf)?,
            Record::Knock(data)   => knock(data, agent, timestamp, buf)?,
            Record::Opaque(..)    => (),
            Record::Ping(data)    => ping(data, agent, timestamp, buf)?,
            Record::Query(data)   => query(data, agent, timestamp, buf)?,
            Record::Shake(data)   => shake(data, agent, timestamp, buf)?,
            Record::Trace(data)   => trace(data, agent, timestamp, buf)?,
            Record::Error(_)      => (),
            Record::Timeout(_)    => (),
        }
    }

    Ok(())
}

fn fetch(data: &Fetch, agent: &str, ts: u128, buf: &mut Vec<u8>) -> Result<()> {
    let addr   = data.addr.to_string();
    let status = i32::from(data.status);
    let rtt    = as_micros(data.rtt);
    let dns    = as_micros(data.dns);
    let tcp    = as_micros(data.tcp);
    let tls    = as_micros(data.tls);
    let size   = i64::try_from(data.size)?;

    Point {
        measurement: "ksynth",
        tags:        &[
            Tag { key: "agent",  value: agent           },
            Tag { key: "task",   value: "fetch"         },
            Tag { key: "target", value: &data.target    },
            Tag { key: "addr",   value: &addr           },
        ],
        fields:      &[
            Field { key: "status", value: status.into() },
            Field { key: "size",   value: size.into()   },
            Field { key: "rtt",    value: rtt.into()    },
            Field { key: "dns",    value: dns.into()    },
            Field { key: "tcp",    value: tcp.into()    },
            Field { key: "tls",    value: tls.into()    },
        ],
        timestamp:   ts,
    }.write(buf);

    Ok(())
}

fn knock(data: &Knock, agent: &str, ts: u128, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let lost = i32::try_from(data.lost)?;
    let sent = i32::try_from(data.sent)?;
    let loss = f64::try_from(lost)? / f64::try_from(sent)?;

    for rtt in &data.result {
        let time = as_micros(*rtt);
        Point {
            measurement: "ksynth",
            tags:        &[
                Tag { key: "agent",  value: agent           },
                Tag { key: "task",   value: "knock"         },
                Tag { key: "target", value: &data.target    },
                Tag { key: "addr",   value: &addr           },
            ],
            fields:      &[
                Field { key: "lost", value: lost.into()     },
                Field { key: "sent", value: sent.into()     },
                Field { key: "loss", value: loss.into()     },
                Field { key: "rtt",  value: time.into()     },
            ],
            timestamp:   ts,
        }.write(buf);
    }

    Ok(())
}

fn ping(data: &Ping, agent: &str, ts: u128, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let lost = i32::try_from(data.lost)?;
    let sent = i32::try_from(data.sent)?;
    let loss = f64::try_from(lost)? / f64::try_from(sent)?;

    for rtt in &data.result {
        let time = as_micros(*rtt);
        Point {
            measurement: "ksynth",
            tags:        &[
                Tag { key: "agent",  value: agent           },
                Tag { key: "task",   value: "ping"          },
                Tag { key: "target", value: &data.target    },
                Tag { key: "addr",   value: &addr           },
            ],
            fields:      &[
                Field { key: "lost", value: lost.into()     },
                Field { key: "sent", value: sent.into()     },
                Field { key: "loss", value: loss.into()     },
                Field { key: "rtt",  value: time.into()     },
            ],
            timestamp:   ts,
        }.write(buf);
    }

    Ok(())
}

fn query(data: &Query, agent: &str, ts: u128, buf: &mut Vec<u8>) -> Result<()> {
    let code = i32::from(data.code);
    let time = as_micros(data.time);

    Point {
        measurement: "ksynth",
        tags:        &[
            Tag { key: "agent",  value: agent        },
            Tag { key: "task",   value: "query"      },
        ],
        fields:      &[
            Field { key: "code",  value: code.into() },
            Field { key: "rtt",   value: time.into() },
        ],
        timestamp:   ts,
    }.write(buf);

    Ok(())
}

fn shake(data: &Shake, agent: &str, ts: u128, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let port = format!("{}", data.port);
    let time = as_micros(data.time);

    Point {
        measurement: "ksynth",
        tags:        &[
            Tag { key: "agent",  value: agent        },
            Tag { key: "task",   value: "shake"      },
            Tag { key: "target", value: &data.target },
            Tag { key: "addr",   value: &addr        },
            Tag { key: "port",   value: &port        },
        ],
        fields:      &[
            Field { key: "rtt",  value: time.into()  },
        ],
        timestamp:   ts,
    }.write(buf);

    Ok(())
}

fn trace(data: &Trace, agent: &str, ts: u128, buf: &mut Vec<u8>) -> Result<()> {
    let addr = data.addr.to_string();
    let time = as_micros(data.time);
    let hops = data.hops.iter().map(|hop| hop.hop).max();
    let hops = i64::try_from(hops.unwrap_or_default())?;

    Point {
        measurement: "ksynth",
        tags:        &[
            Tag { key: "agent",  value: agent        },
            Tag { key: "task",   value: "trace"      },
            Tag { key: "target", value: &data.target },
            Tag { key: "addr",   value: &addr        },
        ],
        fields:      &[
            Field { key: "hops", value: hops.into()  },
            Field { key: "rtt",  value: time.into()  },
        ],
        timestamp:   ts,
    }.write(buf);

    Ok(())
}

fn as_micros(d: Duration) -> i32 {
    i32::try_from(d.as_micros()).unwrap_or(0)
}
