use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error,Result}  ;
use futures::{Stream, StreamExt, TryStreamExt};
use futures::stream::unfold;
use log::{debug, warn};
use rand::random;
use tokio::time::{delay_for, timeout};
use netdiag::{self, Pinger};
use crate::export::{record, Envoy};
use super::resolve;

pub struct Ping {
    id:     u64,
    target: String,
    period: Duration,
    envoy:  Envoy,
    pinger: Arc<Pinger>,
}

impl Ping {
    pub fn new(id: u64, target: String, envoy: Envoy, pinger: Arc<Pinger>) -> Self {
        let period = Duration::from_secs(10);
        Self { id, target, period, envoy, pinger }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: target {}", self.id, self.target);

            let expiry = Duration::from_secs(1);
            let result = self.ping();

            match timeout(expiry, result).await {
                Ok(Ok(rtt)) => self.success(rtt).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            delay_for(self.period).await;
        }
    }

    async fn ping(&self) -> Result<Stats> {
        let pinger = self.pinger.clone();

        let addr  = resolve(&self.target).await?;
        let count = 10;

        let rtt  = ping(pinger, addr).take(count).try_collect::<Vec<_>>().await?;
        let sent = rtt.len();
        let rtt  = rtt.into_iter().flatten().collect::<Vec<_>>();

        let zero = Duration::from_secs(0);
        let min  = rtt.iter().min().unwrap_or(&zero);
        let max  = rtt.iter().max().unwrap_or(&zero);
        let sum  = rtt.iter().sum::<Duration>();

        let avg = sum.checked_div(sent as u32).unwrap_or(zero);

        let sent  = sent  as f64;
        let count = count as f64;
        let loss  = (sent - count) / sent * 100.0;

        Ok(Stats {
            addr: IpAddr::V4(addr),
            min:  *min,
            max:  *max,
            avg:  avg,
            loss: loss,
        })
    }

    async fn success(&self, stats: Stats) {
        debug!("{}: {}", self.id, stats);
        self.envoy.export(record::Ping {
            id:   self.id,
            addr: stats.addr,
            min:  stats.min,
            max:  stats.max,
            avg:  stats.avg,
            loss: stats.loss,
        }).await;
    }

    async fn failure(&self, err: Error) {
        warn!("{}: error: {}", self.id, err);
        self.envoy.export(record::Error {
            id:    self.id,
            cause: err.to_string(),
        }).await;
    }

    async fn timeout(&self) {
        warn!("{}: timeout", self.id);
        self.envoy.export(record::Timeout {
            id: self.id,
        }).await;
    }
}

fn ping(pinger: Arc<Pinger>, addr: Ipv4Addr) -> impl Stream<Item = Result<Option<Duration>>> {
    unfold((pinger, addr, 0), |(pinger, addr, seq)| async move {
        let expiry = Duration::from_millis(250);
        let ident  = random();
        let ping   = netdiag::Ping::new(addr, ident, seq);

        let rtt = match timeout(expiry, pinger.ping(&ping)).await {
            Ok(Ok(rtt)) => Ok(Some(rtt)),
            Ok(Err(e))  => Err(e),
            Err(_)      => Ok(None),
        };

        Some((rtt, (pinger, addr, seq.wrapping_add(1))))
    })
}

#[derive(Debug)]
struct Stats {
    addr: IpAddr,
    min:  Duration,
    max:  Duration,
    avg:  Duration,
    loss: f64,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { min, max, avg, loss, .. } = self;
        write!(f, "min rtt {:.2?}, max {:.2?}, avg {:.2?}, loss: {:0.2}%", min, max, avg, loss)
    }
}
