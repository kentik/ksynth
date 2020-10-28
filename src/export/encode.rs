use std::convert::TryFrom;
use std::net::IpAddr;
use std::time::Duration;
use anyhow::{anyhow, Result};
use capnp::{message, serialize_packed};
use crate::chf_capnp::{c_h_f::Builder, packed_c_h_f};
use super::{Customs, Record, Target, record::*};

pub fn encode(target: &Target, rs: &[Record]) -> Result<Vec<u8>> {
    let cs = Columns::new(target)?;

    let agent = target.agent;

    let mut msg = message::Builder::new_default();
    let root = msg.init_root::<packed_c_h_f::Builder>();
    let mut msgs = root.init_msgs(rs.len() as u32);

    for (index, record) in rs.iter().enumerate() {
        let mut msg = msgs.reborrow().get(index as u32);

        msg.set_sample_rate(1);
        msg.set_sample_adj(true);

        match IpAddr::V4(0.into()) {
            IpAddr::V4(ip) => msg.set_ipv4_src_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_src_addr(&ip.octets()),
        };

        match record {
            Record::Fetch(data)   => cs.fetch(msg, agent, data),
            Record::Knock(data)   => cs.knock(msg, agent, data),
            Record::Ping(data)    => cs.ping(msg, agent, data),
            Record::Query(data)   => cs.query(msg, agent, data),
            Record::Trace(data)   => cs.trace(msg, agent, data),
            Record::Error(data)   => cs.error(msg, agent, data),
            Record::Timeout(data) => cs.timeout(msg, agent, data),
        };
    }

    let mut vec = Vec::new();
    vec.resize_with(80, Default::default);
    serialize_packed::write_message(&mut vec, &msg)?;

    Ok(vec)
}

struct Columns {
    app:    u32,
    agent:  u32,
    kind:   u32,
    task:   u32,
    test:   u32,
    cause:  u32,
    status: u32,
    ttlb:   u32,
    size:   u32,
    sent:   u32,
    lost:   u32,
    rtt:    Stats,
    route:  u32,
    time:   u32,
    port:   u32,
    data:   u32,
    record: u32,
}

struct Stats {
    min: u32,
    max: u32,
    avg: u32,
    std: u32,
    jit: u32,
}

impl Columns {
    fn new(target: &Target) -> Result<Self> {
        let columns = &target.device.columns;
        let lookup  = |name: &str| {
            match columns.iter().find(|c| c.name == name) {
                Some(c) => Ok(c.id as u32),
                None    => Err(anyhow!("missing column '{}'", name)),
            }
        };

        Ok(Self {
            app:     lookup("APP_PROTOCOL")?,
            agent:   lookup("INT64_00")?,
            kind:    lookup("INT00")?,
            task:    lookup("INT64_01")?,
            test:    lookup("INT64_02")?,
            cause:   lookup("STR00")?,
            status:  lookup("INT01")?,
            ttlb:    lookup("INT02")?,
            size:    lookup("INT03")?,
            sent:    lookup("INT01")?,
            lost:    lookup("INT02")?,
            rtt: Stats {
                min: lookup("INT03")?,
                max: lookup("INT04")?,
                avg: lookup("INT05")?,
                std: lookup("INT06")?,
                jit: lookup("INT07")?,
            },
            route:   lookup("STR00")?,
            time:    lookup("INT01")?,
            port:    lookup("INT08")?,
            data:    lookup("STR00")?,
            record:  lookup("STR01")?,
        })
    }

    fn fetch(&self, mut msg: Builder, agent: u64, data: &Fetch) {
        let Fetch { task, test, addr, status, rtt, size, .. } = *data;

        let size = u32::try_from(size).unwrap_or(0);

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("fetch", msg, 8);
        customs.next(self.app,    |v| v.set_uint32_val(AGENT));
        customs.next(self.agent,  |v| v.set_uint64_val(agent));
        customs.next(self.kind,   |v| v.set_uint32_val(FETCH));
        customs.next(self.task,   |v| v.set_uint64_val(task));
        customs.next(self.test,   |v| v.set_uint64_val(test));
        customs.next(self.status, |v| v.set_uint32_val(status.into()));
        customs.next(self.ttlb,   |v| v.set_uint32_val(as_micros(rtt)));
        customs.next(self.size,   |v| v.set_uint32_val(size));
    }

    fn knock(&self, mut msg: Builder, agent: u64, data: &Knock) {
        let Knock { task, test, addr, port, sent, lost, rtt, .. } = *data;

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("knock", msg,  13);
        customs.next(self.app,     |v| v.set_uint32_val(AGENT));
        customs.next(self.agent,   |v| v.set_uint64_val(agent));
        customs.next(self.kind,    |v| v.set_uint32_val(KNOCK));
        customs.next(self.task,    |v| v.set_uint64_val(task));
        customs.next(self.test,    |v| v.set_uint64_val(test));
        customs.next(self.port,    |v| v.set_uint32_val(port.into()));
        customs.next(self.sent,    |v| v.set_uint32_val(sent));
        customs.next(self.lost,    |v| v.set_uint32_val(lost));
        customs.next(self.rtt.min, |v| v.set_uint32_val(as_micros(rtt.min)));
        customs.next(self.rtt.max, |v| v.set_uint32_val(as_micros(rtt.max)));
        customs.next(self.rtt.avg, |v| v.set_uint32_val(as_micros(rtt.avg)));
        customs.next(self.rtt.std, |v| v.set_uint32_val(as_micros(rtt.std)));
        customs.next(self.rtt.jit, |v| v.set_uint32_val(as_micros(rtt.jit)));
    }

    fn ping(&self, mut msg: Builder, agent: u64, data: &Ping) {
        let Ping { task, test, addr, sent, lost, rtt, .. } = *data;

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("ping", msg,  12);
        customs.next(self.app,     |v| v.set_uint32_val(AGENT));
        customs.next(self.agent,   |v| v.set_uint64_val(agent));
        customs.next(self.kind,    |v| v.set_uint32_val(PING));
        customs.next(self.task,    |v| v.set_uint64_val(task));
        customs.next(self.test,    |v| v.set_uint64_val(test));
        customs.next(self.sent,    |v| v.set_uint32_val(sent));
        customs.next(self.lost,    |v| v.set_uint32_val(lost));
        customs.next(self.rtt.min, |v| v.set_uint32_val(as_micros(rtt.min)));
        customs.next(self.rtt.max, |v| v.set_uint32_val(as_micros(rtt.max)));
        customs.next(self.rtt.avg, |v| v.set_uint32_val(as_micros(rtt.avg)));
        customs.next(self.rtt.std, |v| v.set_uint32_val(as_micros(rtt.std)));
        customs.next(self.rtt.jit, |v| v.set_uint32_val(as_micros(rtt.jit)));
    }

    fn query(&self, msg: Builder, agent: u64, data: &Query) {
        let Query { task, test, time, .. } = *data;
        let record  = &data.record;
        let answers = &data.answers;

        let mut customs = Customs::new("query", msg, 8);
        customs.next(self.app,    |v| v.set_uint32_val(AGENT));
        customs.next(self.agent,  |v| v.set_uint64_val(agent));
        customs.next(self.kind,   |v| v.set_uint32_val(QUERY));
        customs.next(self.task,   |v| v.set_uint64_val(task));
        customs.next(self.test,   |v| v.set_uint64_val(test));
        customs.next(self.data,   |v| v.set_str_val(answers));
        customs.next(self.record, |v| v.set_str_val(record));
        customs.next(self.time,   |v| v.set_uint32_val(as_micros(time)));
    }

    fn trace(&self, mut msg: Builder, agent: u64, data: &Trace) {
        let Trace { task, test, addr, time, .. } = *data;

        let route = &data.route;

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("trace", msg, 7);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(TRACE));
        customs.next(self.task,  |v| v.set_uint64_val(task));
        customs.next(self.test,  |v| v.set_uint64_val(test));
        customs.next(self.route, |v| v.set_str_val(route));
        customs.next(self.time,  |v| v.set_uint32_val(as_micros(time)));
    }

    fn error(&self, msg: Builder, agent: u64, data: &Error) {
        let mut customs = Customs::new("error", msg, 6);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(ERROR));
        customs.next(self.task,  |v| v.set_uint64_val(data.task));
        customs.next(self.test,  |v| v.set_uint64_val(data.test));
        customs.next(self.cause, |v| v.set_str_val(&data.cause));
    }

    fn timeout(&self, msg: Builder, agent: u64, data: &Timeout) {
        let mut customs = Customs::new("timeout", msg, 5);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(TIMEOUT));
        customs.next(self.task,  |v| v.set_uint64_val(data.task));
        customs.next(self.test,  |v| v.set_uint64_val(data.test));
    }
}

fn as_micros(d: Duration) -> u32 {
    u32::try_from(d.as_micros()).unwrap_or(0)
}

const AGENT:   u32 = 10;

const ERROR:   u32 = 0;
const TIMEOUT: u32 = 1;
const PING:    u32 = 2;
const FETCH:   u32 = 3;
const TRACE:   u32 = 4;
const KNOCK:   u32 = 5;
const QUERY:   u32 = 6;
