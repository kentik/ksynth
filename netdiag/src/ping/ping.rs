use std::convert::{TryFrom, TryInto};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};
use anyhow::Result;
use etherparse::{Ipv4Header, IpTrafficClass};
use log::{debug, error};
use tokio::sync::{Mutex, oneshot::channel};
use rand::prelude::*;
use raw_socket::{Domain, Type, Protocol};
use raw_socket::tokio::{RawSocket, RawSend, RawRecv};
use crate::Bind;
use crate::icmp::{ping4, ping6, IcmpV4Packet, IcmpV6Packet};
use super::pong::Pong;
use super::state::State;

#[derive(Debug)]
pub struct Ping {
    pub addr:  IpAddr,
    pub id:    u16,
    pub seq:   u16,
    pub token: Token,
}

impl Ping {
    pub fn new(addr: IpAddr, id: u16, seq: u16) -> Self {
        let token = Token(random());
        Self { addr, id, seq, token }
    }
}

pub struct Pinger {
    sock4: Mutex<RawSend>,
    sock6: Mutex<RawSend>,
    state: State,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Token([u8; 16]);

impl Pinger {
    pub async fn new(bind: &Bind) -> Result<Self> {
        let state = State::default();

        let raw   = Type::raw();
        let icmp4 = Protocol::icmpv4();
        let icmp6 = Protocol::icmpv6();

        let sock4 = RawSocket::new(Domain::ipv4(), raw, Some(icmp4))?;
        let sock6 = RawSocket::new(Domain::ipv6(), raw, Some(icmp6))?;

        sock4.bind(bind.sa4()).await?;
        sock6.bind(bind.sa6()).await?;

        let (rx, tx) = sock4.split();
        let sock4 = Mutex::new(tx);
        spawn("recv4", recv4(rx, state.clone()));

        let (rx, tx) = sock6.split();
        let sock6 = Mutex::new(tx);
        spawn("recv6", recv6(rx, state.clone()));

        Ok(Self { sock4, sock6, state })
    }

    pub async fn ping(&self, ping: &Ping) -> Result<Duration> {
        let state = self.state.clone();
        let token = ping.token;

        let (tx, rx) = channel();
        state.insert(token, tx);

        let sent = send(&self.sock4, &self.sock6, ping).await?;
        Pong::new(rx, sent, state, token).await
    }
}

async fn send(sock4: &Mutex<RawSend>, sock6: &Mutex<RawSend>, ping: &Ping) -> Result<Instant> {
    let Ping { addr, id, seq, token } = *ping;

    let mut pkt = [0u8; 64];
    let (sock, pkt) = match addr {
        IpAddr::V4(..) => (sock4, ping4(&mut pkt, id, seq, &token.0)?),
        IpAddr::V6(..) => (sock6, ping6(&mut pkt, id, seq, &token.0)?),
    };

    let addr = SocketAddr::new(addr, 0);
    let mut sock = sock.lock().await;
    sock.send_to(&pkt, &addr).await?;

    Ok(Instant::now())
}

async fn recv4(mut sock: RawRecv, state: State) -> Result<()> {
    let mut pkt = [0u8; 128];
    loop {
        let (n, _) = sock.recv_from(&mut pkt).await?;

        let now = Instant::now();
        let pkt = Ipv4Header::read_from_slice(&pkt[..n])?;

        if let (Ipv4Header { protocol: ICMP4, .. }, tail) = pkt {
            if let IcmpV4Packet::EchoReply(echo) = IcmpV4Packet::try_from(tail)? {
                if let Ok(token) = echo.data.try_into().map(Token) {
                    if let Some(tx) = state.remove(&token) {
                        let _ = tx.send(now);
                    }
                }
            }
        }
    }
}

async fn recv6(mut sock: RawRecv, state: State) -> Result<()> {
    let mut pkt = [0u8; 64];
    loop {
        let (n, _) = sock.recv_from(&mut pkt).await?;

        let now = Instant::now();
        let pkt = IcmpV6Packet::try_from(&pkt[..n])?;

        if let IcmpV6Packet::EchoReply(echo) = pkt {
            if let Ok(token) = echo.data.try_into().map(Token) {
                if let Some(tx) = state.remove(&token) {
                    let _ = tx.send(now);
                }
            }
        }
    }
}

fn spawn<F: Future<Output = Result<()>> + Send + 'static>(name: &'static str, future: F) {
    tokio::spawn(async move {
        match future.await {
            Ok(()) => debug!("{} finished", name),
            Err(e) => error!("{} failed: {}", name, e),
        }
    });
}

const ICMP4: u8 = IpTrafficClass::Icmp as u8;
