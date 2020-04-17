use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Instant, Duration};
use anyhow::{anyhow, Result};
use etherparse::{Ipv4Header, IpTrafficClass};
use futures::future;
use futures::{StreamExt, TryStreamExt};
use libc::IPPROTO_RAW;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, oneshot::channel};
use tokio::time::timeout;
use tokio_raw::{Domain, Type, Protocol, RawSocket};
use crate::icmp::{IcmpV4Packet, Unreachable};
use super::probe::Probe;
use super::reply::{Echo, Node, Reply};
use super::route::Route;
use super::state::State;

#[derive(Debug)]
pub struct Trace {
    pub addr:   Ipv4Addr,
    pub probes: usize,
    pub limit:  usize,
    pub expiry: Duration,
}

pub struct Tracer {
    sock:  Mutex<RawSocket>,
    route: Mutex<UdpSocket>,
    state: Arc<State>,
}

impl Tracer {
    pub async fn new() -> Result<Self> {
        let ipv4 = Domain::ipv4();
        let icmp = Protocol::icmpv4();
        let raw  = Protocol::from(IPPROTO_RAW);

        let icmp  = RawSocket::new(ipv4, Type::raw(), Some(icmp))?;
        let sock  = RawSocket::new(ipv4, Type::raw(), Some(raw))?;
        let route = UdpSocket::bind("0.0.0.0:0").await?;

        let state = Arc::new(State::new());

        tokio::spawn(recv(icmp, state.clone()));

        Ok(Self {
            sock:  Mutex::new(sock),
            route: Mutex::new(route),
            state: state,
        })
    }

    pub async fn route(&self, trace: Trace) -> Result<Vec<Vec<Node>>> {
        let Trace { addr, probes, limit, expiry } = trace;
        let src = self.source(addr).await?;
        let (src, dst) = self.state.reserve(src, addr).await;

        let route = Route::new(self, *src, dst, expiry);

        let mut done = false;
        Ok(route.trace(probes).take_while(|result| {
            let last = done;
            if let Ok(nodes) = result {
                done = nodes.iter().any(|node| {
                    match node {
                        Node::Node(_, ip, _) => ip == &addr,
                        Node::None(_)        => false,
                    }
                });
            }
            future::ready(!last)
        }).take(limit).try_collect().await?)
    }

    pub async fn probe(&self, probe: Probe, expiry: Duration) -> Result<Node> {
        let state = self.state.clone();

        let (tx, rx) = channel();
        state.insert(&probe, tx).await;

        let sent = send(&self.sock, &probe).await?;
        let echo = timeout(expiry, rx);
        Reply::new(echo, sent, state, probe).await
    }

    async fn source(&self, dst: Ipv4Addr) -> Result<Ipv4Addr> {
        let route = self.route.lock().await;
        route.connect(SocketAddr::new(IpAddr::V4(dst), 1234)).await?;
        match route.local_addr()? {
            SocketAddr::V4(sa) => Ok(*sa.ip()),
            SocketAddr::V6(..) => Err(anyhow!("unsupported IPv6 addr")),
        }
    }
}

async fn send(sock: &Mutex<RawSocket>, probe: &Probe) -> Result<Instant> {
    let mut pkt = [0u8; 64];

    let pkt = probe.encode(&mut pkt)?;
    let dst = &probe.dst;

    let mut sock = sock.lock().await;
    sock.send_to(&pkt, &dst).await?;

    Ok(Instant::now())
}

async fn recv(mut sock: RawSocket, state: Arc<State>) -> Result<()> {
    let mut pkt = [0u8; 64];
    loop {
        let (n, from) = sock.recv_from(&mut pkt).await?;

        let from = match from {
            SocketAddr::V4(sa) => *sa.ip(),
            SocketAddr::V6(..) => continue,
        };

        let now = Instant::now();
        let pkt = Ipv4Header::read_from_slice(&pkt[..n])?;

        if let (Ipv4Header { protocol: ICMP, .. }, tail) = pkt {
            let icmp = IcmpV4Packet::try_from(tail)?;

            if let IcmpV4Packet::TimeExceeded(pkt) = icmp {
                if let Some(probe) = Probe::decode(pkt)? {
                    if let Some(tx) = state.remove(&probe).await {
                        let _ = tx.send(Echo(from, now, false));
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

                if let Some(probe) = Probe::decode(pkt)? {
                    if let Some(tx) = state.remove(&probe).await {
                        let _ = tx.send(Echo(from, now, true));
                    }
                }
            }
        }
    }
}

const ICMP: u8 = IpTrafficClass::Icmp as u8;
