use std::net::IpAddr;
use anyhow::{anyhow, Result};
use log::trace;
use rand::prelude::*;
use trust_dns_resolver::TokioAsyncResolver;

#[derive(Clone)]
pub struct Resolver {
    resolver: TokioAsyncResolver,
    ip4:      bool,
    ip6:      bool,
}

impl Resolver {
    pub fn new(resolver: TokioAsyncResolver, ip4: bool, ip6: bool) -> Self {
        Self { resolver, ip4, ip6 }
    }

    pub async fn lookup(&self, host: &str) -> Result<IpAddr> {
        let addrs = match (self.ip4, self.ip6) {
            (true,  true ) => self.resolve(host).await?,
            (true,  false) => self.resolve4(host).await?,
            (false, true ) => self.resolve6(host).await?,
            (false, false) => return Err(anyhow!("invalid config")),
        };

        trace!("{}: {:?}", host, addrs);

        match addrs.choose(&mut thread_rng()) {
            Some(addr)       => Ok(*addr),
            None if self.ip4 => Err(anyhow!("no IPv4 addr for {}", host)),
            None if self.ip6 => Err(anyhow!("no IPv6 addr for {}", host)),
            None             => Err(anyhow!("no IP addr for {}", host))
        }
    }

    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>> {
        match self.resolver.lookup_ip(host).await {
            Ok(r)  => Ok(r.iter().map(IpAddr::from).collect()),
            Err(e) => Err(anyhow!("{}: {}", host, e)),
        }
    }

    async fn resolve4(&self, host: &str) -> Result<Vec<IpAddr>> {
        match self.resolver.ipv4_lookup(host).await {
            Ok(r)  => Ok(r.iter().copied().map(IpAddr::from).collect()),
            Err(e) => Err(anyhow!("{}: {}", host, e)),
        }
    }

    async fn resolve6(&self, host: &str) -> Result<Vec<IpAddr>> {
        match self.resolver.ipv6_lookup(host).await {
            Ok(r)  => Ok(r.iter().copied().map(IpAddr::from).collect()),
            Err(e) => Err(anyhow!("{}: {}", host, e)),
        }
    }
}
