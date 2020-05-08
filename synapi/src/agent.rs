use serde::{Deserialize, de::{Deserializer, Error, Unexpected}};

#[derive(Clone, Debug)]
pub struct Agent {
    pub id:  u64,
    pub net: Net,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Net {
    IPv4,
    IPv6,
    Dual,
}

impl<'d> Deserialize<'d> for Net {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        match u64::deserialize(de)?  {
            0 => Ok(Net::IPv4),
            1 => Ok(Net::IPv6),
            2 => Ok(Net::Dual),
            n => Err(Error::invalid_value(Unexpected::Unsigned(n), &"0..2")),
        }
    }
}
