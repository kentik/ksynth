use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct Agent {
    pub id:  u64,
    pub net: Net,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub enum Net {
    #[serde(rename = "V4")]
    IPv4,
    #[serde(rename = "V6")]
    IPv6,
    #[serde(rename = "DUAL")]
    Dual,
}
