use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error,Result}  ;
use futures::{Stream, StreamExt, TryStreamExt};
use futures::stream::unfold;
use log::{debug, warn};
use rand::random;
use tokio::time::{delay_for, timeout};
use netdiag::{self, Pinger};
use synapi::tasks::PingConfig;
use crate::export::{record, Envoy};
use super::resolve;

pub struct Ping {
    id:     u64,
    target: String,
    period: Duration,
    count:  usize,
    expiry: Duration,
    envoy:  Envoy,
    pinger: Arc<Pinger>,
}

impl Ping {
    pub fn new(id: u64, cfg: PingConfig, envoy: Envoy, pinger: Arc<Pinger>) -> Self {
        let PingConfig { target, period, count, expiry } = cfg;

        let period = Duration::from_secs(period);
        let count  = count as usize;
        let expiry = Duration::from_millis(expiry);

        Self { id, target, period, count, expiry, envoy, pinger }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: target {}", self.id, self.target);

            let result = self.ping(self.count);

            match timeout(self.expiry, result).await {
                Ok(Ok(rtt)) => self.success(rtt).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            delay_for(self.period).await;
        }
    }

    async fn ping(&self, count: usize) -> Result<Stats> {
        let pinger = self.pinger.clone();

        let addr = IpAddr::V4(resolve(&self.target).await?);

        let rtt  = ping(pinger, addr).take(count).try_collect::<Vec<_>>().await?;
        let sent = rtt.len() as u32;
        let rtt  = rtt.into_iter().flatten().collect::<Vec<_>>();
        let lost = sent - rtt.len() as u32;

        let zero = Duration::from_secs(0);
        let min  = rtt.iter().min().unwrap_or(&zero);
        let max  = rtt.iter().max().unwrap_or(&zero);
        let sum  = rtt.iter().sum::<Duration>();

        let avg = sum.checked_div(sent).unwrap_or(zero);

        Ok(Stats {
            addr: addr,
            sent: sent,
            lost: lost,
            min:  *min,
            max:  *max,
            avg:  avg,
        })
    }

    async fn success(&self, stats: Stats) {
        debug!("{}: {}", self.id, stats);
        self.envoy.export(record::Ping {
            id:   self.id,
            addr: stats.addr,
            sent: stats.sent,
            lost: stats.lost,
            min:  stats.min,
            max:  stats.max,
            avg:  stats.avg,
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

fn ping(pinger: Arc<Pinger>, addr: IpAddr) -> impl Stream<Item = Result<Option<Duration>>> {
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
    sent: u32,
    lost: u32,
    min:  Duration,
    max:  Duration,
    avg:  Duration,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { sent, lost, min, max, avg, .. } = self;
        let good = sent - lost;
        write!(f, "{}/{} min rtt {:.2?}, max {:.2?}, avg {:.2?}", good, sent, min, max, avg)
    }
}
