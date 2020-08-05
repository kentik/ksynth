use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use clap::{value_t, values_t};
use rand::prelude::*;
use tokio::time::{delay_for, timeout};
use netdiag::{Bind, Pinger, Ping};
use crate::args::Args;
use super::resolve;

pub async fn ping(args: Args<'_, '_>) -> Result<()> {
    let count  = value_t!(args, "count",  u16)?;
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

    let pinger = Arc::new(Pinger::new(&bind).await?);

    let delay  = Duration::from_millis(delay);
    let expiry = Duration::from_millis(expiry);

    for (host, addr) in resolve(hosts, ip4, ip6).await {
        println!("ping {} ({})", host, addr);

        let ident = random();

        for n in 0..count {
            let ping = Ping::new(addr, ident, n);

            match timeout(expiry, pinger.ping(&ping)).await {
                Ok(Ok(d))  => println!("seq {} RTT {:0.2?} ", n, d),
                Ok(Err(e)) => println!("seq {} error: {:?} ", n, e),
                Err(_)     => println!("seq {} timeout", n),
            };

            delay_for(delay).await;
        }
    }

    Ok(())
}
