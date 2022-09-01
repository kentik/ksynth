use std::convert::{TryFrom, TryInto};
use anyhow::{anyhow, Error, Result};
use synapi::tasks::Kind;

#[derive(Debug)]
pub enum Value<'a> {
    String(&'a str),
    UInt32(u32),
    UInt64(u64),
}

impl<'a> TryFrom<(Kind, &'a serde_json::Value)> for Value<'a> {
    type Error = Error;

    fn try_from((kind, value): (Kind, &'a serde_json::Value)) -> Result<Self> {
        match kind {
            Kind::String => Ok(Value::String(string(value)?)),
            Kind::UInt32 => Ok(Value::UInt32(uint32(value)?)),
            Kind::UInt64 => Ok(Value::UInt64(uint64(value)?)),
            _            => Err(anyhow!("unsupported type: {kind:?}")),
        }
    }
}

fn string(value: &serde_json::Value) -> Result<&str> {
    match value.as_str() {
        Some(n) => Ok(n),
        None    => Err(anyhow!("invalid str: {value:?}")),
    }
}

fn uint32(value: &serde_json::Value) -> Result<u32> {
    match value.as_u64() {
        Some(n) => Ok(n.try_into()?),
        None    => Err(anyhow!("invalid u32: {value:?}")),
    }
}

fn uint64(value: &serde_json::Value) -> Result<u64> {
    match value.as_u64() {
        Some(n) => Ok(n),
        None    => Err(anyhow!("invalid u64: {value:?}")),
    }
}
