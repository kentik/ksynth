use rustls::RootCertStore;
use netdiag::Bind;
use crate::export::Envoy;
use super::Resolver;

pub struct Task {
    pub task:     u64,
    pub test:     u64,
    pub network:  Network,
    pub envoy:    Envoy,
    pub resolver: Resolver,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Network {
    IPv4,
    IPv6,
    Dual,
}

#[derive(Clone)]
pub struct Config {
    pub bind:     Bind,
    pub network:  Option<Network>,
    pub resolver: Resolver,
    pub roots:    RootCertStore,
}

impl Task {
    pub fn new(task: u64, test: u64, network: Network, envoy: Envoy, resolver: Resolver) -> Self {
        Self { task, test, network, envoy, resolver }
    }
}
