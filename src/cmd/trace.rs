use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use anyhow::Result;
use clap::{value_t, values_t, ArgMatches};
use tokio::net::UdpSocket;
use tokio::time::delay_for;
use netdiag::{Node, Probe, Tracer};
use super::resolve;

pub async fn trace(args: &ArgMatches<'_>) -> Result<()> {
    let delay  = value_t!(args, "delay",  u64)?;
    let expiry = value_t!(args, "expiry", u64)?;
    let limit  = value_t!(args, "limit",  usize)?;
    let probes = value_t!(args, "probes", usize)?;
    let ip4    = true;
    let ip6    = false;
    let hosts  = values_t!(args, "host", String)?;

    let delay  = Duration::from_millis(delay);
    let expiry = Duration::from_millis(expiry);

    let tracer = Tracer::new().await?;
    let source = UdpSocket::bind("0.0.0.0:0").await?;

    for (host, addr) in resolve(hosts, ip4, ip6).await {
        println!("trace {} ({})", host, addr);

        let mut dst = match addr {
            IpAddr::V4(ip) => SocketAddrV4::new(ip, 33434),
            IpAddr::V6(..) => panic!("IPv6 not supported"),
        };

        source.connect(SocketAddr::V4(dst)).await?;

        let src = match source.local_addr()? {
            SocketAddr::V4(sa) => sa,
            SocketAddr::V6(..) => panic!("IPv6 not supported"),
        };

        for ttl in 1..=limit {
            let mut nodes = HashMap::<IpAddr, Vec<String>>::new();

            for _ in 0..probes {
                let probe = Probe::new(src, dst, ttl as u8);
                let node  = tracer.probe(probe, expiry).await?;

                if let Node::Node(_, addr, rtt) = node {
                    let addr = IpAddr::V4(addr);
                    let rtt  = format!("{:>0.2?}", rtt);
                    nodes.entry(addr).or_default().push(rtt);
                }

                dst.set_port(dst.port() + 1);

                delay_for(delay).await;
            }

            print(&nodes, ttl, probes);

            if nodes.contains_key(&addr) {
                break;
            }
        }
    }

    Ok(())
}

fn print(nodes: &HashMap<IpAddr, Vec<String>>, ttl: usize, probes: usize) {
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
