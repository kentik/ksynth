use serde::{Deserialize, de::Deserializer};

#[derive(Debug)]
pub struct Okay;

impl<'d> Deserialize<'d> for Okay {
    fn deserialize<D: Deserializer<'d>>(_de: D) -> Result<Self, D::Error> {
        Ok(Okay)
    }
}
