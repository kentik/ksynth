use std::convert::TryFrom;
use std::net::{IpAddr, SocketAddr};
use std::time::Instant;
use std::sync::Arc;
use anyhow::Result;
use etherparse::{Ipv4Header, IpTrafficClass};
use libc::{IPPROTO_RAW, c_int};
use raw_socket::tokio::prelude::*;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use crate::Bind;
use crate::icmp::{IcmpV4Packet, icmp4::Unreachable};
use super::probe::ProbeV4;
use super::reply::Echo;
use super::state::State;

pub struct Sock4 {
    sock:  Mutex<RawSocket>,
    route: Mutex<UdpSocket>,
}

impl Sock4 {
    pub async fn new(bind: &Bind, state: Arc<State>) -> Result<Self> {
        let ipv4 = Domain::ipv4();
        let icmp = Protocol::icmpv4();
        let raw  = Protocol::from(IPPROTO_RAW);

        let icmp  = RawSocket::new(ipv4, Type::raw(), Some(icmp))?;
        let sock  = RawSocket::new(ipv4, Type::raw(), Some(raw))?;
        let route = UdpSocket::bind(bind.sa4()).await?;

        sock.bind(bind.sa4()).await?;

        let enable: c_int = 6;
        sock.set_sockopt(Level::IPV4, Name::IPV4_HDRINCL, &enable)?;

        tokio::spawn(recv(icmp, state));

        Ok(Self {
            sock:  Mutex::new(sock),
            route: Mutex::new(route),
        })
    }

    pub async fn send(&self, probe: &ProbeV4) -> Result<Instant> {
        let mut pkt = [0u8; 64];

        let pkt = probe.encode(&mut pkt)?;
        let dst = &probe.dst;

        let mut sock = self.sock.lock().await;
        sock.send_to(&pkt, dst).await?;

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
        let pkt = Ipv4Header::read_from_slice(&pkt[..n])?;

        if let (Ipv4Header { protocol: ICMP, .. }, tail) = pkt {
            let icmp = IcmpV4Packet::try_from(tail)?;

            if let IcmpV4Packet::TimeExceeded(pkt) = icmp {
                if let Some(probe) = ProbeV4::decode(pkt)? {
                    if let Some(tx) = state.remove(&probe) {
                        let _ = tx.send(Echo(from.ip(), now, false));
                    }
                }
            } else if let IcmpV4Packet::Unreachable(what) = icmp {
                let pkt = match what {
                    Unreachable::Net(pkt)      => pkt,
                    Unreachable::Host(pkt)     => pkt,
                    Unreachable::Protocol(pkt) => pkt,
                    Unreachable::Port(pkt)     => pkt,
                    Unreachable::Other(_, pkt) => pkt,
                };

                if let Some(probe) = ProbeV4::decode(pkt)? {
                    if let Some(tx) = state.remove(&probe) {
                        let _ = tx.send(Echo(from.ip(), now, true));
                    }
                }
            }
        }
    }
}

const ICMP: u8 = IpTrafficClass::Icmp as u8;
