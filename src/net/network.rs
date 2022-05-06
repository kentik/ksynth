use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    IPv4,
    IPv6,
    Dual,
}

impl Network {
    pub fn includes(self, net: Network) -> bool {
        self == Network::Dual || self == net
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::Dual
    }
}
