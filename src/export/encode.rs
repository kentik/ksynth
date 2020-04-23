use std::net::IpAddr;
use anyhow::{anyhow, Result};
use capnp::{message, serialize_packed};
use crate::chf_capnp::{c_h_f::Builder, packed_c_h_f};
use super::{Customs, Record, Target, record::*};

pub fn encode(target: &Target, rs: &[Record]) -> Result<Vec<u8>> {
    let cs = Columns::new(target)?;

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
            Record::Fetch(data)   => cs.fetch(msg, data),
            Record::Ping(data)    => cs.ping(msg, data),
            Record::Trace(data)   => cs.trace(msg, data),
            Record::Error(data)   => cs.error(msg, data),
            Record::Timeout(data) => cs.timeout(msg, data),
        };
    }

    let mut vec = Vec::new();
    vec.resize_with(80, Default::default);
    serialize_packed::write_message(&mut vec, &msg)?;

    Ok(vec)
}

struct Columns {
    kind:  u32,
    id:    u32,
    cause: u32,
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
            kind:  lookup("APP_PROTOCOL")?,
            id:    lookup("INT64_00")?,
            cause: lookup("STR00")?,
        })
    }

    fn fetch(&self, mut msg: Builder, data: &Fetch) {
        match data.addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new(msg.init_custom(2));
        customs.next(self.kind, |v| v.set_uint32_val(0));
        customs.next(self.id,   |v| v.set_uint64_val(data.id));
    }

    fn ping(&self, mut msg: Builder, data: &Ping) {
        match data.addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new(msg.init_custom(2));
        customs.next(self.kind, |v| v.set_uint32_val(0));
        customs.next(self.id,   |v| v.set_uint64_val(data.id));
    }

    fn trace(&self, mut msg: Builder, data: &Trace) {
        match data.addr {
            IpAddr::V4(ip) => msg.set_ipv4_dst_addr(ip.into()),
            IpAddr::V6(ip) => msg.set_ipv6_dst_addr(&ip.octets()),
        };

        let mut customs = Customs::new(msg.init_custom(2));
        customs.next(self.kind, |v| v.set_uint32_val(0));
        customs.next(self.id,   |v| v.set_uint64_val(data.id));
    }

    fn error(&self, msg: Builder, data: &Error) {
        let mut customs = Customs::new(msg.init_custom(3));
        customs.next(self.kind,  |v| v.set_uint32_val(0));
        customs.next(self.id,    |v| v.set_uint64_val(data.id));
        customs.next(self.cause, |v| v.set_str_val(&data.cause));
    }

    fn timeout(&self, msg: Builder, data: &Timeout) {
        let mut customs = Customs::new(msg.init_custom(2));
        customs.next(self.kind, |v| v.set_uint32_val(0));
        customs.next(self.id,   |v| v.set_uint64_val(data.id));
    }
}
