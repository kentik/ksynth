use std::time::Duration;
use serde::{Serialize, ser::{Serializer, SerializeMap}};

#[derive(Debug)]
pub struct Metric<'a> {
    pub name:       &'a str,
    pub value:      Value,
    pub timestamp:  Duration,
    pub attributes: &'a [Attribute<'a>],
}

#[derive(Debug)]
pub enum Value {
    Count(u64, Duration),
    Gauge(f64),
}

#[derive(Debug, Default)]
pub struct Attributes<'a>(pub &'a [Attribute<'a>]);

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Attribute<'a> {
    String(&'a str, &'a str),
    Number(&'a str, u64),
    Boolean(&'a str, bool),
}

impl<'a> Metric<'a> {
    pub fn count(name: &'a str, value: u64, ts: Duration, interval: Duration) -> Self {
        Self {
            name:       name,
            value:      Value::Count(value, interval),
            timestamp:  ts,
            attributes: &[],
        }
    }

    pub fn gauge(name: &'a str, value: f64, ts: Duration) -> Self {
        Self {
            name:       name,
            value:      Value::Gauge(value),
            timestamp:  ts,
            attributes: &[],
        }
    }
}

impl Serialize for Metric<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Self { name, value, timestamp, attributes } = self;

        match value {
            Value::Count(v, i) => serialize_count(serializer, name, v, i, timestamp, attributes),
            Value::Gauge(v)    => serialize_gauge(serializer, name, v, timestamp, attributes),
        }
    }
}

fn serialize_count<S: Serializer>(
    serializer: S,
    name:       &str,
    value:      &u64,
    interval:   &Duration,
    timestamp:  &Duration,
    attributes: &[Attribute<'_>],
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(6))?;
    map.serialize_entry("name", name)?;
    map.serialize_entry("type", "count")?;
    map.serialize_entry("value", value)?;
    map.serialize_entry("interval.ms", &interval.as_millis())?;
    map.serialize_entry("timestamp", &timestamp.as_secs())?;
    map.serialize_entry("attributes", &Attributes(attributes))?;
    map.end()
}

fn serialize_gauge<S: Serializer>(
    serializer: S,
    name:       &str,
    value:      &f64,
    timestamp:  &Duration,
    attributes: &[Attribute<'_>],
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(5))?;
    map.serialize_entry("name", name)?;
    map.serialize_entry("type", "gauge")?;
    map.serialize_entry("value", value)?;
    map.serialize_entry("timestamp", &timestamp.as_secs())?;
    map.serialize_entry("attributes", &Attributes(attributes))?;
    map.end()
}

impl Serialize for Attributes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for attr in self.0 {
            match attr {
                Attribute::String(name, value)  => map.serialize_entry(name, value)?,
                Attribute::Number(name, value)  => map.serialize_entry(name, value)?,
                Attribute::Boolean(name, value) => map.serialize_entry(name, value)?,
            }
        }
        map.end()
    }
}
