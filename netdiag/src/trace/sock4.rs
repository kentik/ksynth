use std::net::{IpAddr, SocketAddr};
use std::time::Instant;
use std::sync::Arc;
use anyhow::Result;
use etherparse::{Ipv4Header, IpTrafficClass, TcpHeaderSlice};
use libc::{IPPROTO_TCP, IPPROTO_UDP, c_int};
use raw_socket::tokio::prelude::*;
use raw_socket::tokio::{RawRecv, RawSend};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use crate::Bind;
use super::probe::{Key, Probe};
use super::reply::Echo;
use super::state::State;

pub struct Sock4 {
    tcp:   Mutex<RawSend>,
    udp:   Mutex<RawSend>,
    route: Mutex<UdpSocket>,
}

impl Sock4 {
    pub async fn new(bind: &Bind, state: Arc<State>) -> Result<Self> {
        let ipv4  = Domain::ipv4();
        let tcp   = Protocol::from(IPPROTO_TCP);
        let udp   = Protocol::from(IPPROTO_UDP);

        let tcp   = RawSocket::new(ipv4, Type::raw(), Some(tcp))?;
        let udp   = RawSocket::new(ipv4, Type::raw(), Some(udp))?;
        let route = UdpSocket::bind(bind.sa4()).await?;

        tcp.bind(bind.sa4()).await?;
        udp.bind(bind.sa4()).await?;

        let enable: c_int = 6;
        tcp.set_sockopt(Level::IPV4, Name::IPV4_HDRINCL, &enable)?;
        udp.set_sockopt(Level::IPV4, Name::IPV4_HDRINCL, &enable)?;
        let (rx, tcp) = tcp.split();
        let (_,  udp) = udp.split();

        tokio::spawn(recv(rx, state));

        Ok(Self {
            tcp:   Mutex::new(tcp),
            udp:   Mutex::new(udp),
            route: Mutex::new(route),
        })
    }

    pub async fn send(&self, probe: &Probe, ttl: u8) -> Result<Instant> {
        let mut pkt = [0u8; 64];

        let pkt = probe.encode(&mut pkt, ttl)?;
        let dst = probe.dst();

        match probe {
            Probe::TCP(..) => self.tcp.lock().await,
            Probe::UDP(..) => self.udp.lock().await,
        }.send_to(&pkt, &dst).await?;

        Ok(Instant::now())
    }

    pub async fn source(&self, dst: IpAddr) -> Result<IpAddr> {
        let route = self.route.lock().await;
        route.connect(SocketAddr::new(dst, 1234)).await?;
        Ok(route.local_addr()?.ip())
    }
}

async fn recv(mut sock: RawRecv, state: Arc<State>) -> Result<()> {
    let mut pkt = [0u8; 64];
    loop {
        let (n, from) = sock.recv_from(&mut pkt).await?;

        let now = Instant::now();
        let pkt = Ipv4Header::read_from_slice(&pkt[..n])?;

        if let (ip @ Ipv4Header { protocol: TCP, .. }, tail) = pkt {
            let src = IpAddr::V4(ip.source.into());
            let dst = IpAddr::V4(ip.destination.into());

            let pkt = TcpHeaderSlice::from_slice(&tail)?;
            let src = SocketAddr::new(src, pkt.source_port());
            let dst = SocketAddr::new(dst, pkt.destination_port());
            let key = Key(dst, src);

            if let Some(tx) = state.remove(&key) {
                let _ = tx.send(Echo(from.ip(), now, true));
            }
        }
    }
}

const TCP: u8 = IpTrafficClass::Tcp as u8;
