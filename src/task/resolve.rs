use std::net::{Ipv4Addr, SocketAddr};
use anyhow::{anyhow, Result};
use rand::prelude::*;
use tokio::net::lookup_host;

pub async fn resolve(host: &str) -> Result<Ipv4Addr> {
    let addr = format!("{}:0", host);

    let addrs = lookup_host(&addr).await?.flat_map(|addr| {
        match addr {
            SocketAddr::V4(addr) => Some(addr),
            SocketAddr::V6(_)    => None,
        }
    }).collect::<Vec<_>>();

    match addrs.choose(&mut thread_rng()) {
        Some(addr) => Ok(*addr.ip()),
        None       => Err(anyhow!("no IPv4 addr for {}", host))
    }
}
