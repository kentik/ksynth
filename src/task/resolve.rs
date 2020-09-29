use std::net::IpAddr;
use anyhow::{anyhow, Result};
use log::trace;
use rand::prelude::*;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::error::{ResolveError, ResolveErrorKind};
use super::Network;

#[derive(Clone)]
pub struct Resolver {
    resolver: TokioAsyncResolver,
}

impl Resolver {
    pub fn new(resolver: TokioAsyncResolver) -> Self {
        Self { resolver }
    }

    pub async fn lookup(&self, host: &str, net: Network) -> Result<IpAddr> {
        let addrs = match net {
            Network::Dual => self.resolve(host).await?,
            Network::IPv4 => self.resolve4(host).await?,
            Network::IPv6 => self.resolve6(host).await?,
        };

        trace!("{}: {:?}", host, addrs);

        match addrs.choose(&mut thread_rng()) {
            Some(addr)                   => Ok(*addr),
            None if net == Network::IPv4 => Err(anyhow!("no IPv4 addr for {}", host)),
            None if net == Network::IPv6 => Err(anyhow!("no IPv6 addr for {}", host)),
            None                         => Err(anyhow!("no IP addr for {}", host))
        }
    }

    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>> {
        match self.resolver.lookup_ip(host).await {
            Ok(r)  => Ok(r.iter().map(IpAddr::from).collect()),
            Err(e) => result(host, e),
        }
    }

    async fn resolve4(&self, host: &str) -> Result<Vec<IpAddr>> {
        match self.resolver.ipv4_lookup(host).await {
            Ok(r)  => Ok(r.iter().copied().map(IpAddr::from).collect()),
            Err(e) => result(host, e),
        }
    }

    async fn resolve6(&self, host: &str) -> Result<Vec<IpAddr>> {
        match self.resolver.ipv6_lookup(host).await {
            Ok(r)  => Ok(r.iter().copied().map(IpAddr::from).collect()),
            Err(e) => result(host, e)
        }
    }
}

fn result(host: &str, e: ResolveError) -> Result<Vec<IpAddr>> {
    match e.kind() {
        ResolveErrorKind::NoRecordsFound { .. } => Ok(Vec::new()),
        _                                       => Err(anyhow!("{}: {}", host, e)),
    }
}
