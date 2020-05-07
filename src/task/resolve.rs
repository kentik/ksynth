use std::net::{IpAddr, SocketAddr};
use anyhow::{anyhow, Result};
use rand::prelude::*;
use tokio::net::lookup_host;

pub async fn resolve(host: &str, ip4: bool, ip6: bool) -> Result<IpAddr> {
    let addr = format!("{}:0", host);

    let select = |addr: SocketAddr| match addr {
        sa if sa.is_ipv4() && ip4 => Some(sa.ip()),
        sa if sa.is_ipv6() && ip6 => Some(sa.ip()),
        _                         => None,
    };

    let addrs: Vec<IpAddr> = match lookup_host(&addr).await {
        Ok(addrs) => addrs.flat_map(select).collect(),
        Err(e)    => return Err(anyhow!("{}: {}", host, e)),
    };

    match addrs.choose(&mut thread_rng()) {
        Some(addr)  => Ok(*addr),
        None if ip4 => Err(anyhow!("no IPv4 addr for {}", host)),
        None if ip6 => Err(anyhow!("no IPv6 addr for {}", host)),
        None        => Err(anyhow!("no IP addr for {}", host))
    }
}
