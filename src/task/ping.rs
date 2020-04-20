use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use futures::{Stream, StreamExt, TryStreamExt};
use futures::stream::unfold;
use log::{debug, warn};
use rand::random;
use tokio::time::{delay_for, timeout};
use netdiag::{self, Pinger};

pub struct Ping {
    id:     u64,
    addr:   Ipv4Addr,
    period: Duration,
    pinger: Arc<Pinger>,
}

impl Ping {
    pub fn new(id: u64, addr: Ipv4Addr, pinger: Arc<Pinger>) -> Self {
        let period = Duration::from_secs(10);
        Self { id, addr, period, pinger }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: target {}", self.id, self.addr);

            let pinger = self.pinger.clone();
            let addr   = self.addr;
            let count  = 10;
            let expiry = Duration::from_secs(1);

            let rtt = ping(pinger, addr).take(count).try_collect::<Vec<_>>();

            match timeout(expiry, rtt).await {
                Ok(Ok(rtt)) => self.report(rtt),
                Ok(Err(e))  => warn!("{}", e),
                Err(_)      => warn!("timeout"),
            };

            delay_for(self.period).await;
        }
    }

    fn report(&self, rtt: Vec<Option<Duration>>) {
        let zero  = Duration::from_secs(0);
        let first = rtt.iter().flatten().next();

        let mut min   = *first.unwrap_or(&zero);
        let mut max   = zero;
        let mut sum   = zero;
        let mut count = 0;

        for &rtt in rtt.iter().flatten() {
            min    = min.min(rtt);
            max    = max.max(rtt);
            sum   += rtt;
            count += 1;
        }

        let avg   = sum.checked_div(count as u32).unwrap_or(zero);

        let sent  = rtt.len() as f64;
        let count = count as f64;
        let loss  = (sent - count) / sent * 100.0;

        debug!("{}: min rtt {:.2?}, max {:.2?}, avg {:.2?}, loss: {:0.2}%", self.id, min, max, avg, loss);
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
