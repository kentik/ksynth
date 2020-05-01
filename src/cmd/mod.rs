use std::net::IpAddr;
use anyhow::Result;
use clap::ArgMatches;
use futures::stream::{self, StreamExt};
use rand::prelude::*;
use tokio::net::lookup_host;
use tokio::runtime::Runtime;

pub fn ping(args: &ArgMatches<'_>) -> Result<()> {
    Runtime::new()?.block_on(ping::ping(args))
}

pub async fn resolve(hosts: Vec<String>, ip4: bool, ip6: bool) -> Vec<(String, IpAddr)> {
    stream::iter(hosts).filter_map(|host| async move {
        match lookup(&host, ip4, ip6).await {
            Ok(Some(addr))  => return Some((host, addr)),
            Ok(None) if ip4 => println!("no IPv4 addr for {}", host),
            Ok(None) if ip6 => println!("no IPv6 addr for {}", host),
            Ok(None)        => println!("no IP addr for {}", host),
            Err(e)          => println!("host {}: {}", host, e),
        }
        None
    }).collect().await
}

pub async fn lookup(host: &str, ip4: bool, ip6: bool) -> Result<Option<IpAddr>> {
    let addr = format!("{}:0", host);

    let addrs = lookup_host(&addr).await?.flat_map(|addr| {
        match addr {
            sa if sa.is_ipv4() && ip4 => Some(sa.ip()),
            sa if sa.is_ipv6() && ip6 => Some(sa.ip()),
            _                         => None,
        }
    }).collect::<Vec<_>>();

    Ok(addrs.into_iter().choose(&mut thread_rng()))
}

mod ping;
