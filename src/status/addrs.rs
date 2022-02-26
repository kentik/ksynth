use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use bytes::Buf;
use http::Method;
use hyper::body::aggregate;
use log::warn;
use nix::ifaddrs::{getifaddrs, InterfaceAddress};
use nix::sys::socket::SockAddr;
use parking_lot::Mutex;
use serde::Deserialize;
use tokio::task::spawn_blocking;
use tokio::time::{interval, timeout};
use crate::net::Network;
use crate::net::http::{HttpClient, Request};
use crate::task::Config;

pub struct Addresses {
    client:  HttpClient,
    network: Network,
}

impl Addresses {
    pub fn new(config: Config) -> Result<Self> {
        let Config { bind, network, resolver, roots, .. } = config;
        let client  = HttpClient::new(bind, resolver, roots)?;
        let network = network.unwrap_or(Network::Dual);
        Ok(Self { client, network })
    }

    pub async fn exec(self, addrs: Arc<Mutex<Vec<IpAddr>>>) {
        let mut ticker = interval(Duration::from_secs(90));

        loop {
            ticker.tick().await;

            match self.scan().await {
                Ok(list) => *addrs.lock() = list,
                Err(e)   => warn!("{e:?}"),
            }
        }
    }

    pub async fn scan(&self) -> Result<Vec<IpAddr>> {
        let expiry = Duration::from_secs(5);

        let local = spawn_blocking(local);
        let ipv4  = timeout(expiry, self.probe(Network::IPv4));
        let ipv6  = timeout(expiry, self.probe(Network::IPv6));
        let (ipv4, ipv6, local) = tokio::join!(ipv4, ipv6, local);

        let mut addrs = local??;

        if self.network.includes(Network::IPv4) {
            match ipv4 {
                Ok(Ok(ip)) => addrs.push(ip),
                Ok(Err(e)) => warn!("IPv4 discovery error: {e:?}"),
                Err(_)     => warn!("IPv4 discovery timeout"),
            }
        }

        if self.network.includes(Network::IPv6) {
            match ipv6 {
                Ok(Ok(ip)) => addrs.push(ip),
                Ok(Err(e)) => warn!("IPv6 discovery error: {e:?}"),
                Err(_)     => warn!("IPv6 discovery timeout"),
            }
        }

        addrs.dedup();
        Ok(addrs)
    }

    async fn probe(&self, network: Network) -> Result<IpAddr> {
        #[derive(Debug, Deserialize)]
        struct Response {
            address: IpAddr,
        }

        let endpoint = "https://whoami.kentiklabs.com".parse()?;
        let request  = Request::new(network, Method::GET, endpoint)?;
        let response = self.client.request(request).await?;

        let body     = aggregate(response.body).await?.reader();
        let response = serde_json::from_reader::<_, Response>(body)?;

        Ok(response.address)
    }
}

fn local() -> Result<Vec<IpAddr>> {
    let mut addrs = Vec::new();
    for InterfaceAddress { address, .. } in getifaddrs()? {
        if let Some(SockAddr::Inet(inet)) = address {
            let ip = inet.ip().to_std();
            match ip {
                IpAddr::V4(v4) if !v4.is_local() => addrs.push(ip),
                IpAddr::V6(v6) if !v6.is_local() => addrs.push(ip),
                _                                => (),
            }
        }
    }
    Ok(addrs)
}

trait IsLocal {
    fn is_local(&self) -> bool;
}

impl IsLocal for Ipv4Addr {
    fn is_local(&self) -> bool {
        self.is_loopback()
            || self.is_broadcast()
            || self.is_link_local()
            || self.is_private()
    }
}

impl IsLocal for Ipv6Addr {
    fn is_local(&self) -> bool {
        self.is_loopback()
            || self.is_multicast()
            || is_unicast_link_local(self)
            || is_unique_local(self)
    }
}

fn is_unicast_link_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn is_unique_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}
