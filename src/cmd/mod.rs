use std::net::IpAddr;
use anyhow::Result;
use futures::stream::{self, StreamExt};
use crate::args::{App, Args};
use crate::net::{Network, Resolver};

pub fn knock(app: App, args: Args<'_, '_>) -> Result<()> {
    app.runtime.block_on(knock::knock(args))
}

pub fn ping(app: App, args: Args<'_, '_>) -> Result<()> {
    app.runtime.block_on(ping::ping(args))
}

pub fn trace(app: App, args: Args<'_, '_>) -> Result<()> {
    app.runtime.block_on(trace::trace(args))
}

pub fn ctl(app: App, args: Args<'_, '_>) -> Result<()> {
    app.runtime.block_on(ctl::ctl(args))
}

pub async fn resolve(resolver: &Resolver, hosts: Vec<String>, net: Network) -> Vec<(String, IpAddr)> {
    stream::iter(hosts).filter_map(|host| async move {
        match resolver.lookup(&host, net).await {
            Ok(addr) => return Some((host, addr)),
            Err(e)   => println!("{}", e),
        }
        None
    }).collect().await
}

mod ctl;
mod knock;
mod ping;
mod trace;
