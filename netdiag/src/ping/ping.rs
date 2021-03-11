use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use futures::{StreamExt, TryStreamExt};
use futures::stream::unfold;
use rand::random;
use tokio::time::timeout;
use crate::Bind;
use super::{sock4::Sock4, sock6::Sock6};
use super::probe::Probe;
use super::state::State;

#[derive(Debug)]
pub struct Ping {
    pub addr:   IpAddr,
    pub count:  usize,
    pub expiry: Duration,
}

pub struct Pinger {
    sock4: Sock4,
    sock6: Sock6,
    state: Arc<State>,
}

impl Pinger {
    pub async fn new(bind: &Bind) -> Result<Self> {
        let state = Arc::new(State::default());

        let sock4 = Sock4::new(bind, state.clone()).await?;
        let sock6 = Sock6::new(bind, state.clone()).await?;

        Ok(Self { sock4, sock6, state })
    }

    pub async fn ping(&self, ping: Ping) -> Result<Vec<Option<Duration>>> {
        let Ping { addr, count, expiry } = ping;

        unfold((addr, 0), |(addr, seq)| async move {
            let ident = random();
            let probe = Probe::new(addr, ident, seq);
            let rtt   = self.probe(&probe, expiry).await;
            Some((rtt, (addr, seq.wrapping_add(1))))
        }).take(count).try_collect::<Vec<_>>().await
    }

    pub async fn probe(&self, probe: &Probe, expiry: Duration) -> Result<Option<Duration>> {
        let rx   = self.state.insert(probe.token);
        let sent = self.send(probe).await?;

        Ok(match timeout(expiry, rx).await {
            Ok(r)  => Some(r?.saturating_duration_since(sent)),
            Err(_) => None,
        })
    }

    pub async fn send(&self, probe: &Probe) -> Result<Instant> {
        match probe.addr {
            IpAddr::V4(_) => self.sock4.send(probe).await,
            IpAddr::V6(_) => self.sock6.send(probe).await,
        }
    }
}
