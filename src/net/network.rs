#[derive(Clone, Copy, Debug, PartialEq)]
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
