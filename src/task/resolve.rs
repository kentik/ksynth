use std::net::IpAddr;
use anyhow::{anyhow, Result};
use log::trace;
use rand::prelude::*;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::error::{ResolveError, ResolveErrorKind};
use {super::Network, super::Network::*};

#[derive(Clone)]
pub struct Resolver {
    resolver: TokioAsyncResolver,
}

impl Resolver {
    pub fn new(resolver: TokioAsyncResolver) -> Self {
        Self { resolver }
    }

    pub async fn lookup(&self, host: &str, net: Network) -> Result<IpAddr> {
        if let Ok(ip) = host.parse::<IpAddr>() {
            match net {
                IPv4 if ip.is_ipv4() => return Ok(ip),
                IPv6 if ip.is_ipv6() => return Ok(ip),
                Dual                 => return Ok(ip),
                _                    => ()
            }
        }

        let addrs = match net {
            Dual => self.resolve(host).await?,
            IPv4 => self.resolve4(host).await?,
            IPv6 => self.resolve6(host).await?,
        };

        trace!("{}: {:?}", host, addrs);

        match addrs.choose(&mut thread_rng()) {
            Some(addr)          => Ok(*addr),
            None if net == IPv4 => Err(anyhow!("no IPv4 addr for {}", host)),
            None if net == IPv6 => Err(anyhow!("no IPv6 addr for {}", host)),
            None                => Err(anyhow!("no IP addr for {}", host))
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

#[cfg(test)]
mod test {
    use std::future::Future;
    use anyhow::Result;
    use tokio::runtime::Builder;
    use trust_dns_resolver::TokioAsyncResolver;
    use trust_dns_resolver::system_conf::read_system_conf;
    use super::*;

    #[test]
    fn resolve_dual() -> Result<()> {
        test(async {
            let resolver = resolver()?;
            assert!(resolver.lookup("8.8.8.8",    Dual).await.is_ok());
            assert!(resolver.lookup("fd00::1",    Dual).await.is_ok());
            assert!(resolver.lookup("google.com", Dual).await.is_ok());
            Ok(())
        })
    }

    #[test]
    fn resolve_ipv4() -> Result<()> {
        test(async {
            let resolver = resolver()?;
            assert!(resolver.lookup("8.8.8.8",    IPv4).await.is_ok());
            assert!(resolver.lookup("google.com", IPv4).await.is_ok());
            Ok(())
        })
    }

    #[test]
    fn resolve_ipv6() -> Result<()> {
        test(async {
            let resolver = resolver()?;
            assert!(resolver.lookup("fd00::1",    IPv6).await.is_ok());
            assert!(resolver.lookup("google.com", IPv6).await.is_ok());
            Ok(())
        })
    }

    #[test]
    fn resolve_error() -> Result<()> {
        test(async {
            let resolver = resolver()?;
            assert!(resolver.lookup("fd00::1", IPv4).await.is_err());
            assert!(resolver.lookup("8.8.8.8", IPv6).await.is_err());
            Ok(())
        })
    }

    fn test(future: impl Future<Output = Result<()>>) -> Result<()> {
        Builder::new_current_thread().enable_all().build()?.block_on(future)
    }

    fn resolver() -> Result<Resolver> {
        let (config, options) = read_system_conf()?;
        let resolver = TokioAsyncResolver::tokio(config, options)?;
        Ok(Resolver::new(resolver))
    }
}
