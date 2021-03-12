use std::time::Duration;
use anyhow::Result;
use clap::{value_t, values_t};
use futures::{pin_mut, StreamExt};
use tokio::time::sleep;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::system_conf::read_system_conf;
use netdiag::{Bind, Ping, Pinger};
use crate::args::Args;
use crate::task::{Network, Resolver};
use super::resolve;

pub async fn ping(args: Args<'_, '_>) -> Result<()> {
    let count  = value_t!(args, "count",  usize)?;
    let delay  = value_t!(args, "delay",  u64)?;
    let expiry = value_t!(args, "expiry", u64)?;
    let ip4    = !args.is_present("ip6");
    let ip6    = !args.is_present("ip4");
    let hosts  = values_t!(args, "host", String)?;

    let mut bind = Bind::default();
    if let Some(addrs) = args.values_of("bind") {
        for addr in addrs {
            bind.set(addr.parse()?);
        }
    }

    let net = match (ip4, ip6) {
        (true, false) => Network::IPv4,
        (false, true) => Network::IPv6,
        _             => Network::Dual,
    };

    let (config, options) = read_system_conf()?;
    let resolver = TokioAsyncResolver::tokio(config, options)?;
    let resolver = Resolver::new(resolver.clone());

    let pinger = Pinger::new(&bind).await?;

    let delay  = Duration::from_millis(delay);
    let expiry = Duration::from_millis(expiry);

    for (host, addr) in resolve(&resolver, hosts, net).await {
        println!("ping {} ({})", host, addr);

        let ping   = Ping { addr, count, expiry };
        let stream = pinger.ping(&ping).enumerate();
        pin_mut!(stream);

        while let Some((n, item)) = stream.next().await {
            match item? {
                Some(d) => println!("seq {} RTT {:0.2?} ", n, d),
                None    => println!("seq {} timeout", n),
            }
            sleep(delay).await;
        }
    }

    Ok(())
}
