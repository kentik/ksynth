use std::convert::{TryFrom, TryInto};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use anyhow::Result;
use etherparse::{Ipv4Header, IpTrafficClass};
use tokio::sync::{Mutex, oneshot::channel};
use tokio_raw::{Domain, Type, Protocol, RawSocket, RawSend, RawRecv};
use rand::prelude::*;
use crate::icmp::{self, IcmpV4Packet};
use super::pong::Pong;
use super::state::State;

#[derive(Debug)]
pub struct Ping {
    pub addr:  Ipv4Addr,
    pub id:    u16,
    pub seq:   u16,
    pub token: Token,
}

impl Ping {
    pub fn new(addr: Ipv4Addr, id: u16, seq: u16) -> Self {
        let token = Token(random());
        Self { addr, id, seq, token }
    }
}

pub struct Pinger {
    sock:  Mutex<RawSend>,
    state: State,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Token([u8; 16]);

impl Pinger {
    pub fn new() -> Result<Self> {
        let ipv4 = Domain::ipv4();
        let raw  = Type::raw();
        let icmp = Protocol::icmpv4();

        let sock = RawSocket::new(ipv4, raw, Some(icmp))?;

        let (rx, tx) = sock.split();
        let sock  = Mutex::new(tx);
        let state = State::default();

        tokio::spawn(recv(rx, state.clone()));

        Ok(Self { sock, state })
    }

    pub async fn ping(&self, ping: &Ping) -> Result<Duration> {
        let state = self.state.clone();
        let token = ping.token;

        let (tx, rx) = channel();
        state.insert(ping.token, tx).await;

        let sent = send(&self.sock, ping).await?;
        Pong::new(rx, sent, state, token).await
    }
}

async fn send(sock: &Mutex<RawSend>, ping: &Ping) -> Result<Instant> {
    let mut pkt = [0u8; 64];

    let Ping { addr, id, seq, token } = *ping;
    let pkt  = icmp::ping(&mut pkt, id, seq, &token.0)?;
    let addr = SocketAddr::new(IpAddr::V4(addr), 0);

    let mut sock = sock.lock().await;
    sock.send_to(&pkt, &addr).await?;

    Ok(Instant::now())
}

async fn recv(mut sock: RawRecv, state: State) -> Result<()> {
    let mut pkt = [0u8; 64];
    loop {
        let (n, _) = sock.recv_from(&mut pkt).await?;

        let now = Instant::now();
        let pkt = Ipv4Header::read_from_slice(&pkt[..n])?;

        if let (Ipv4Header { protocol: ICMP, .. }, tail) = pkt {
            if let IcmpV4Packet::EchoReply(echo) = IcmpV4Packet::try_from(tail)? {
                if let Ok(token) = echo.data.try_into().map(Token) {
                    if let Some(tx) = state.remove(&token).await {
                        let _ = tx.send(now);
                    }
                }
            }
        }
    }
}

const ICMP: u8 = IpTrafficClass::Icmp as u8;
