use anyhow::Result;
use serde_json::{Map, Value};

pub struct Machine;

impl Machine {
    pub fn invoke(&self, _args: Value) -> Result<Value> {
        let mut data = Map::new();
        data.insert("INT00".into(), 11.into());
        Ok(data.into())
    }
}
