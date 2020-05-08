use std::net::IpAddr;
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
            Record::Ping(data)    => cs.ping(msg, agent, data),
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
    cause:  u32,
    status: u32,
    rtt:    u32,
    size:   u32,
    sent:   u32,
    lost:   u32,
    min:    u32,
    max:    u32,
    avg:    u32,
    route:  u32,
    time:   u32,
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
            app:    lookup("APP_PROTOCOL")?,
            agent:  lookup("INT64_00")?,
            kind:   lookup("INT00")?,
            task:   lookup("INT64_01")?,
            cause:  lookup("STR00")?,
            status: lookup("INT01")?,
            rtt:    lookup("INT64_02")?,
            size:   lookup("INT64_03")?,
            sent:   lookup("INT01")?,
            lost:   lookup("INT02")?,
            min:    lookup("INT64_02")?,
            max:    lookup("INT64_03")?,
            avg:    lookup("INT64_04")?,
            route:  lookup("STR00")?,
            time:   lookup("INT64_02")?,
        })
    }

    fn fetch(&self, mut msg: Builder, agent: u64, data: &Fetch) {
        let Fetch { id, addr, status, rtt, size, .. } = *data;

        let rtt  = rtt.as_micros() as u64;
        let size = size            as u64;

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("fetch", msg, 7);
        customs.next(self.app,    |v| v.set_uint32_val(AGENT));
        customs.next(self.agent,  |v| v.set_uint64_val(agent));
        customs.next(self.kind,   |v| v.set_uint32_val(FETCH));
        customs.next(self.task,   |v| v.set_uint64_val(id));
        customs.next(self.status, |v| v.set_uint16_val(status));
        customs.next(self.rtt,    |v| v.set_uint64_val(rtt));
        customs.next(self.size,   |v| v.set_uint64_val(size));
    }

    fn ping(&self, mut msg: Builder, agent: u64, data: &Ping) {
        let Ping { id, addr, sent, lost, min, max, avg, .. } = *data;

        let min = min.as_micros() as u64;
        let max = max.as_micros() as u64;
        let avg = avg.as_micros() as u64;

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("ping", msg,  9);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(PING));
        customs.next(self.task,  |v| v.set_uint64_val(id));
        customs.next(self.sent,  |v| v.set_uint32_val(sent));
        customs.next(self.lost,  |v| v.set_uint32_val(lost));
        customs.next(self.min,   |v| v.set_uint64_val(min));
        customs.next(self.max,   |v| v.set_uint64_val(max));
        customs.next(self.avg,   |v| v.set_uint64_val(avg));
    }

    fn trace(&self, mut msg: Builder, agent: u64, data: &Trace) {
        let Trace { id, addr, time, .. } = *data;

        let route = &data.route;
        let time  = time.as_micros() as u64;

        match addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new("trace", msg, 6);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(TRACE));
        customs.next(self.task,  |v| v.set_uint64_val(id));
        customs.next(self.route, |v| v.set_str_val(route));
        customs.next(self.time,  |v| v.set_uint64_val(time));
    }

    fn error(&self, msg: Builder, agent: u64, data: &Error) {
        let mut customs = Customs::new("error", msg, 5);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(ERROR));
        customs.next(self.task,  |v| v.set_uint64_val(data.id));
        customs.next(self.cause, |v| v.set_str_val(&data.cause));
    }

    fn timeout(&self, msg: Builder, agent: u64, data: &Timeout) {
        let mut customs = Customs::new("timeout", msg, 4);
        customs.next(self.app,   |v| v.set_uint32_val(AGENT));
        customs.next(self.agent, |v| v.set_uint64_val(agent));
        customs.next(self.kind,  |v| v.set_uint32_val(TIMEOUT));
        customs.next(self.task,  |v| v.set_uint64_val(data.id));
    }
}

const AGENT:   u32 = 10;

const ERROR:   u32 = 0;
const TIMEOUT: u32 = 1;
const PING:    u32 = 2;
const FETCH:   u32 = 3;
const TRACE:   u32 = 4;
