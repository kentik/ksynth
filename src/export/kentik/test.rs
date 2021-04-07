use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use capnp::message::ReaderOptions;
use capnp::serialize_packed::try_read_message;
use rand::{thread_rng, Rng};
use synapi::tasks::{Column, Device, Kind};
use crate::chf_capnp::{custom::value::Which, packed_c_h_f};
use crate::stats::Summary;
use crate::export::{Record, Target, record::*};
use super::{encode, encode::*};

#[test]
fn encode_fetch() -> Result<()> {
    let mut rng = thread_rng();

    let record = Fetch::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),         values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent),  values["INT64_00"]);
    assert_eq!(Value::from(record.task),   values["INT64_01"]);
    assert_eq!(Value::from(record.test),   values["INT64_02"]);
    assert_eq!(Value::from(FETCH),         values["INT00"]);
    assert_eq!(Value::from(record.status), values["INT01"]);
    assert_eq!(Value::from(record.rtt),    values["INT02"]);
    assert_eq!(Value::from(record.size),   values["INT03"]);
    assert_eq!(Value::from(record.addr),   dst_addr(record.addr, &values));

    Ok(())
}

#[test]
fn encode_knock() -> Result<()> {
    let mut rng = thread_rng();

    let record = Knock::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),          values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent),   values["INT64_00"]);
    assert_eq!(Value::from(record.task),    values["INT64_01"]);
    assert_eq!(Value::from(record.test),    values["INT64_02"]);
    assert_eq!(Value::from(KNOCK),          values["INT00"]);
    assert_eq!(Value::from(record.sent),    values["INT01"]);
    assert_eq!(Value::from(record.lost),    values["INT02"]);
    assert_eq!(Value::from(record.rtt.min), values["INT03"]);
    assert_eq!(Value::from(record.rtt.max), values["INT04"]);
    assert_eq!(Value::from(record.rtt.avg), values["INT05"]);
    assert_eq!(Value::from(record.rtt.std), values["INT06"]);
    assert_eq!(Value::from(record.rtt.jit), values["INT07"]);
    assert_eq!(Value::from(record.port),    values["INT08"]);
    assert_eq!(Value::from(record.addr),    dst_addr(record.addr, &values));

    Ok(())
}

#[test]
fn encode_ping() -> Result<()> {
    let mut rng = thread_rng();

    let record = Ping::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),          values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent),   values["INT64_00"]);
    assert_eq!(Value::from(record.task),    values["INT64_01"]);
    assert_eq!(Value::from(record.test),    values["INT64_02"]);
    assert_eq!(Value::from(PING),           values["INT00"]);
    assert_eq!(Value::from(record.sent),    values["INT01"]);
    assert_eq!(Value::from(record.lost),    values["INT02"]);
    assert_eq!(Value::from(record.rtt.min), values["INT03"]);
    assert_eq!(Value::from(record.rtt.max), values["INT04"]);
    assert_eq!(Value::from(record.rtt.avg), values["INT05"]);
    assert_eq!(Value::from(record.rtt.std), values["INT06"]);
    assert_eq!(Value::from(record.rtt.jit), values["INT07"]);
    assert_eq!(Value::from(record.addr),    dst_addr(record.addr, &values));

    Ok(())
}

#[test]
fn encode_query() -> Result<()> {
    let mut rng = thread_rng();

    let record = Query::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),           values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent),    values["INT64_00"]);
    assert_eq!(Value::from(record.task),     values["INT64_01"]);
    assert_eq!(Value::from(record.test),     values["INT64_02"]);
    assert_eq!(Value::from(QUERY),           values["INT00"]);
    assert_eq!(Value::from(record.time),     values["INT01"]);
    assert_eq!(Value::from(record.code),     values["INT02"]);
    assert_eq!(Value::from(&record.answers), values["STR00"]);
    assert_eq!(Value::from(&record.record),  values["STR01"]);

    Ok(())
}

#[test]
fn encode_shake() -> Result<()> {
    let mut rng = thread_rng();

    let record = Shake::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),        values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent), values["INT64_00"]);
    assert_eq!(Value::from(record.task),  values["INT64_01"]);
    assert_eq!(Value::from(record.test),  values["INT64_02"]);
    assert_eq!(Value::from(SHAKE),        values["INT00"]);
    assert_eq!(Value::from(record.time),  values["INT01"]);
    assert_eq!(Value::from(record.port),  values["INT08"]);
    assert_eq!(Value::from(record.addr),  dst_addr(record.addr, &values));

    Ok(())
}

#[test]
fn encode_trace() -> Result<()> {
    let mut rng = thread_rng();

    let record = Trace::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),         values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent),  values["INT64_00"]);
    assert_eq!(Value::from(record.task),   values["INT64_01"]);
    assert_eq!(Value::from(record.test),   values["INT64_02"]);
    assert_eq!(Value::from(TRACE),         values["INT00"]);
    assert_eq!(Value::from(record.time),   values["INT01"]);
    assert_eq!(Value::from(&record.route), values["STR00"]);
    assert_eq!(Value::from(record.addr),   dst_addr(record.addr, &values));

    Ok(())
}

#[test]
fn encode_error() -> Result<()> {
    let mut rng = thread_rng();

    let record = Error::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),         values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent),  values["INT64_00"]);
    assert_eq!(Value::from(record.task),   values["INT64_01"]);
    assert_eq!(Value::from(record.test),   values["INT64_02"]);
    assert_eq!(Value::from(ERROR),         values["INT00"]);
    assert_eq!(Value::from(&record.cause), values["STR00"]);

    Ok(())
}

#[test]
fn encode_timeout() -> Result<()> {
    let mut rng = thread_rng();

    let record = Timeout::gen(&mut rng);
    let target = target(&mut rng);
    let values = serde(&target, record.clone())?;

    assert_eq!(Value::from(AGENT),        values["APP_PROTOCOL"]);
    assert_eq!(Value::from(target.agent), values["INT64_00"]);
    assert_eq!(Value::from(record.task),  values["INT64_01"]);
    assert_eq!(Value::from(record.test),  values["INT64_02"]);
    assert_eq!(Value::from(TIMEOUT),      values["INT00"]);

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    AddrV4(Ipv4Addr),
    AddrV6(Ipv6Addr),
    UInt32(u32),
    UInt64(u64),
    String(String),
    Other,
}

impl From<u16> for Value {
    fn from(v: u16) -> Self {
        Value::UInt32(v.into())
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::UInt32(v)
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::UInt64(v)
    }
}

impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::UInt32(u32::try_from(v).unwrap())
    }
}

impl From<Duration> for Value {
    fn from(v: Duration) -> Self {
        Value::UInt32(u32::try_from(v.as_micros()).unwrap())
    }
}

impl From<&String> for Value {
    fn from(v: &String) -> Self {
        Value::String(v.clone())
    }
}

impl From<IpAddr> for Value {
    fn from(v: IpAddr) -> Self {
        match v {
            IpAddr::V4(ip) => Value::AddrV4(ip),
            IpAddr::V6(ip) => Value::AddrV6(ip),
        }
    }
}

fn serde<T: Into<Record>>(target: &Target, record: T) -> Result<HashMap<String, Value>> {
    let data   = encode(target, &vec![record.into()])?;
    let opts   = ReaderOptions::new();
    let cursor = Cursor::new(&data[80..]);
    let reader = try_read_message(cursor, opts)?.unwrap();
    let packed = reader.get_root::<packed_c_h_f::Reader>()?;

    let columns = target.device.columns.iter().map(|c| {
        Ok((u32::try_from(c.id)?, &*c.name))
    }).collect::<Result<HashMap<_, _>>>()?;

    let mut values = HashMap::new();
    for msg in packed.get_msgs()?.iter() {
        let ip4 = Ipv4Addr::from(msg.get_ipv4_dst_addr());
        let ip6 = Ipv6Addr::from(match msg.get_ipv6_dst_addr()? {
            b if b.len() == 16 => b.try_into()?,
            _                  => [0u8; 16],
        });

        values.insert("IPV4_DST_ADDR".to_owned(), Value::AddrV4(ip4));
        values.insert("IPV6_DST_ADDR".to_owned(), Value::AddrV6(ip6.into()));

        for customs in msg.get_custom().iter() {
            for custom in customs.iter() {
                let id    = custom.get_id();
                let name  = columns[&id].to_owned();
                let value = custom.get_value();
                values.insert(name, match value.which()? {
                    Which::Uint32Val(n) => Value::UInt32(n),
                    Which::Uint64Val(n) => Value::UInt64(n),
                    Which::StrVal(s)    => Value::String(s?.to_string()),
                    _                   => Value::Other,
                });
            }
        }
    }

    Ok(values)
}

fn dst_addr(addr: IpAddr, values: &HashMap<String, Value>) -> Value {
    match addr {
        IpAddr::V4(_) => values["IPV4_DST_ADDR"].clone(),
        IpAddr::V6(_) => values["IPV6_DST_ADDR"].clone(),
    }
}

fn target<R: Rng>(rng: &mut R) -> Target {
    let mut columns = Vec::new();
    let mut index   = 1;

    let mut push = |name: String, kind: Kind| {
        columns.push(Column {
            id:   index,
            name: name,
            kind: kind
        });
        index += 1;
    };

    push("APP_PROTOCOL".into(),  Kind::UInt32);
    push("IPV4_DST_ADDR".into(), Kind::Addr);
    push("IPV6_DST_ADDR".into(), Kind::Addr);

    for kind in &[Kind::UInt32, Kind::UInt64, Kind::String] {
        for n in 0..32 {
            push(format!("{}{:02}", match kind {
                Kind::UInt32 => "INT",
                Kind::UInt64 => "INT64_",
                Kind::String => "STR",
                Kind::Addr   => "INET_",
            }, n), *kind);
        }
    }

    let device = Device {
        id:      random(rng),
        columns: columns
    };

    Target {
        company: random(rng),
        agent:   random(rng),
        device:  device,
        email:   String::new(),
        token:   String::new(),
    }
}

trait Random {
    fn gen<R: Rng>(rng: &mut R) -> Self;
}

fn random<T: Random, R: Rng>(rng: &mut R) -> T {
    T::gen(rng)
}

impl Random for Fetch  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:   random(rng),
            test:   random(rng),
            target: Arc::new(random(rng)),
            addr:   random(rng),
            status: random(rng),
            dns:    random(rng),
            tcp:    random(rng),
            tls:    random(rng),
            rtt:    random(rng),
            size:   random(rng),
        }
    }
}

impl Random for Knock  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:   random(rng),
            test:   random(rng),
            target: Arc::new(random(rng)),
            addr:   random(rng),
            port:   random(rng),
            sent:   random(rng),
            lost:   random(rng),
            rtt:    random(rng),
            result: random(rng),
        }
    }
}

impl Random for Ping  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:   random(rng),
            test:   random(rng),
            target: Arc::new(random(rng)),
            addr:   random(rng),
            sent:   random(rng),
            lost:   random(rng),
            rtt:    random(rng),
            result: random(rng),
        }
    }
}

impl Random for Query  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:    random(rng),
            test:    random(rng),
            code:    random(rng),
            record:  random(rng),
            answers: random(rng),
            time:    random(rng),
        }
    }
}

impl Random for Shake  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:   random(rng),
            test:   random(rng),
            target: Arc::new(random(rng)),
            addr:   random(rng),
            port:   random(rng),
            time:   random(rng),
        }
    }
}

impl Random for Trace  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:   random(rng),
            test:   random(rng),
            target: Arc::new(random(rng)),
            addr:   random(rng),
            hops:   random(rng),
            route:  random(rng),
            time:   random(rng),
        }
    }
}

impl Random for Error  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task:  random(rng),
            test:  random(rng),
            cause: random(rng),
        }
    }
}

impl Random for Timeout  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            task: random(rng),
            test: random(rng),
        }
    }
}

impl Random for Hop {
    fn gen<R: Rng>(_rng: &mut R) -> Self {
        Self {
            hop:   0,
            nodes: HashMap::new(),
        }
    }
}

impl Random for Summary  {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Self {
            min: random(rng),
            max: random(rng),
            avg: random(rng),
            std: random(rng),
            jit: random(rng),
        }
    }
}

impl Random for u16 {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }
}

impl Random for u32 {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }
}

impl Random for u64 {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }
}

impl Random for usize {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        usize::try_from(rng.gen::<u32>()).unwrap()
    }
}

impl Random for IpAddr {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        match rng.gen() {
            true  => IpAddr::V4(Ipv4Addr::from(rng.gen::<u32>())),
            false => IpAddr::V6(Ipv6Addr::from(rng.gen::<u128>())),
        }
    }
}

impl Random for Duration {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        Duration::from_micros(u64::from(rng.gen::<u32>()))
    }
}

impl Random for String {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        (0..).flat_map(|_| {
            match rng.gen::<char>() {
                c if c.is_ascii_alphanumeric() => Some(c),
                _                              => None,
            }
        }).take(8).collect()
    }
}

impl<T: Random> Random for Vec<T> {
    fn gen<R: Rng>(rng: &mut R) -> Self {
        (0..8).map(|_| {
            T::gen(rng)
        }).collect()
    }
}
