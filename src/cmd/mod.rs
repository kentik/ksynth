use std::net::IpAddr;
use anyhow::Result;
use clap::ArgMatches;
use futures::stream::{self, StreamExt};
use tokio::runtime::Runtime;
use crate::task;

pub fn ping(args: &ArgMatches<'_>) -> Result<()> {
    Runtime::new()?.block_on(ping::ping(args))
}

pub fn trace(args: &ArgMatches<'_>) -> Result<()> {
    Runtime::new()?.block_on(trace::trace(args))
}

pub async fn resolve(hosts: Vec<String>, ip4: bool, ip6: bool) -> Vec<(String, IpAddr)> {
    stream::iter(hosts).filter_map(|host| async move {
        match task::resolve(&host, ip4, ip6).await {
            Ok(addr) => return Some((host, addr)),
            Err(e)   => println!("{}", e),
        }
        None
    }).collect().await
}

mod ping;
mod trace;
