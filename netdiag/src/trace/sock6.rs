use std::convert::TryFrom;
use std::io::IoSlice;
use std::net::{IpAddr, SocketAddr};
use std::time::Instant;
use std::sync::Arc;
use anyhow::Result;
use libc::c_int;
use raw_socket::tokio::prelude::*;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use crate::Bind;
use crate::icmp::{IcmpV6Packet, icmp6::Unreachable};
use super::probe::ProbeV6;
use super::reply::Echo;
use super::state::State;

pub struct Sock6 {
    sock:  Mutex<RawSocket>,
    route: Mutex<UdpSocket>,
}

impl Sock6 {
    pub async fn new(bind: &Bind, state: Arc<State>) -> Result<Self> {
        let ipv6 = Domain::ipv6();
        let icmp = Protocol::icmpv6();
        let udp  = Protocol::udp();

        let icmp  = RawSocket::new(ipv6, Type::raw(), Some(icmp))?;
        let sock  = RawSocket::new(ipv6, Type::raw(), Some(udp))?;
        let route = UdpSocket::bind(bind.sa6()).await?;

        let offset: c_int = 6;
        sock.set_sockopt(Level::IPV6, Name::IPV6_CHECKSUM, &offset)?;
        sock.bind(bind.sa6()).await?;

        tokio::spawn(recv(icmp, state));

        Ok(Self {
            sock:  Mutex::new(sock),
            route: Mutex::new(route),
        })
    }

    pub async fn send(&self, probe: &ProbeV6) -> Result<Instant> {
        let mut dst = probe.dst;
        let mut ctl = [0u8; 64];
        let mut pkt = [0u8; 64];

        let pkt = probe.encode(&mut pkt)?;
        dst.set_port(0);

        let hops = CMsg::Ipv6HopLimit(probe.ttl as c_int);
        let ctl  = CMsg::encode(&mut ctl, &[hops])?;
        let data = &[IoSlice::new(pkt)];

        let mut sock = self.sock.lock().await;
        sock.send_msg(&dst, data, Some(&ctl)).await?;

        Ok(Instant::now())
    }

    pub async fn source(&self, dst: IpAddr) -> Result<IpAddr> {
        let route = self.route.lock().await;
        route.connect(SocketAddr::new(dst, 1234)).await?;
        Ok(route.local_addr()?.ip())
    }
}

async fn recv(mut sock: RawSocket, state: Arc<State>) -> Result<()> {
    let mut pkt = [0u8; 64];
    loop {
        let (n, from) = sock.recv_from(&mut pkt).await?;

        let now = Instant::now();
        let pkt = IcmpV6Packet::try_from(&pkt[..n])?;

        if let IcmpV6Packet::HopLimitExceeded(pkt) = pkt {
            if let Some(probe) = ProbeV6::decode(pkt)? {
                if let Some(tx) = state.remove(&probe).await {
                    let _ = tx.send(Echo(from.ip(), now, false));
                }
            }
        } else if let IcmpV6Packet::Unreachable(what) = pkt {
            let pkt = match what {
                Unreachable::Address(pkt)  => pkt,
                Unreachable::Port(pkt)     => pkt,
                Unreachable::Other(_, pkt) => pkt,
            };

            if let Some(probe) = ProbeV6::decode(pkt)? {
                if let Some(tx) = state.remove(&probe).await {
                    let _ = tx.send(Echo(from.ip(), now, true));
                }
            }
        }
    }
}
