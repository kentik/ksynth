use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;
use anyhow::Result;
use clap::{value_t, values_t};
use futures::{pin_mut, stream::StreamExt};
use tokio::time::sleep;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::system_conf::read_system_conf;
use netdiag::{Bind, Node, Protocol, Tracer};
use crate::args::Args;
use crate::net::{Network, Resolver};
use super::resolve;

pub async fn trace(args: Args<'_, '_>) -> Result<()> {
    let delay  = value_t!(args, "delay",  u64)?;
    let expiry = value_t!(args, "expiry", u64)?;
    let limit  = value_t!(args, "limit",  u8)?;
    let probes = value_t!(args, "probes", usize)?;
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

    let delay  = Duration::from_millis(delay);
    let expiry = Duration::from_millis(expiry);

    let tracer = Tracer::new(&bind).await?;

    for (host, addr) in resolve(&resolver, hosts, net).await {
        println!("trace {} ({})", host, addr);

        let tcp   = args.opt("tcp")?;
        let icmp  = args.is_present("icmp");
        let proto = match (tcp, icmp) {
            (Some(port), false) => Protocol::TCP(port),
            (None,       true) => Protocol::ICMP,
            _                 => Protocol::default(),
        };

        let source = tracer.reserve(proto, addr).await?;

        let mut done  = false;
        let mut ttl   = 1;
        let mut probe = source.probe()?;

        while !done && ttl <= limit {
            let mut nodes = HashMap::<IpAddr, Vec<String>>::new();

            let stream = tracer.probe(&mut probe, ttl, expiry);
            let stream = stream.take(probes);
            pin_mut!(stream);

            while let Some(Ok(node)) = stream.next().await {
                if let Node::Node(_, ip, rtt, last) = node {
                    let rtt = format!("{:>0.2?}", rtt);
                    nodes.entry(ip).or_default().push(rtt);
                    done = last || ip == addr;
                }

                sleep(delay).await;
            }

            print(&nodes, ttl, probes);

            ttl += 1;
        }
    }

    Ok(())
}

fn print(nodes: &HashMap<IpAddr, Vec<String>>, ttl: u8, probes: usize) {
    let mut count = 0;

    let mut output = nodes.iter().map(|(node, rtt)| {
        count += rtt.len();
        let node = node.to_string();
        let rtt  = rtt.join(", ");
        (node, rtt)
    }).collect::<Vec<_>>();

    if count < probes {
        let node = "* ".repeat(probes - count);
        let rtt  = String::new();
        output.push((node, rtt));
    }

    for (n, (node, rtt)) in output.iter().enumerate() {
        match n {
            0 => println!("[{:>3}] {:32} {}", ttl, node, rtt),
            _ => println!("[{:>3}] {:32} {}", "",  node, rtt),
        }
    }
}
