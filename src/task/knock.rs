use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error,Result}  ;
use log::{debug, warn};
use tokio::time::{delay_for, timeout};
use netdiag::{self, Knocker};
use synapi::tasks::KnockConfig;
use crate::export::{record, Envoy};
use crate::stats::{summarize, Summary};
use super::resolve;

pub struct Knock {
    id:      u64,
    test_id: u64,
    target:  String,
    port:    u16,
    period:  Duration,
    count:   usize,
    expiry:  Duration,
    envoy:   Envoy,
    knocker: Arc<Knocker>,
}

impl Knock {
    pub fn new(id: u64, cfg: KnockConfig, envoy: Envoy, knocker: Arc<Knocker>) -> Self {
        let KnockConfig { test_id, target, port, period, count, expiry } = cfg;

        let period = Duration::from_secs(period);
        let count  = count as usize;
        let expiry = Duration::from_millis(expiry);

        Self { id, test_id, target, port, period, count, expiry, envoy, knocker }
    }

    pub async fn exec(self, ip4: bool, ip6: bool) -> Result<()> {
        loop {
            let Self { id, test_id, target, port, .. } = &self;

            debug!("{}: test {}, target {}:{}", id, test_id, target, port);

            let result = self.knock(self.count, ip4, ip6);

            match timeout(self.expiry, result).await {
                Ok(Ok(rtt)) => self.success(rtt).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            delay_for(self.period).await;
        }
    }

    async fn knock(&self, count: usize, ip4: bool, ip6: bool) -> Result<Output> {
        let knocker = &self.knocker;

        let addr = resolve(&self.target, ip4, ip6).await?;
        let port = self.port;

        let knock = netdiag::Knock {
            addr:   addr,
            port:   port,
            count:  count,
            expiry: Duration::from_millis(500),
        };

        let rtt  = knocker.knock(knock).await?;
        let sent = rtt.len() as u32;
        let rtt  = rtt.into_iter().flatten().collect::<Vec<_>>();
        let lost = sent - rtt.len() as u32;

        Ok(Output {
            addr: addr,
            port: port,
            sent: sent,
            lost: lost,
            rtt:  summarize(&rtt).unwrap_or_default(),
        })
    }

    async fn success(&self, out: Output) {
        debug!("{}: {}", self.id, out);
        self.envoy.export(record::Knock {
            id:      self.id,
            test_id: self.test_id,
            addr:    out.addr,
            port:    out.port,
            sent:    out.sent,
            lost:    out.lost,
            rtt:     out.rtt,
        }).await;
    }

    async fn failure(&self, err: Error) {
        warn!("{}: error: {}", self.id, err);
        self.envoy.export(record::Error {
            id:      self.id,
            test_id: self.test_id,
            cause:   err.to_string(),
        }).await;
    }

    async fn timeout(&self) {
        warn!("{}: timeout", self.id);
        self.envoy.export(record::Timeout {
            id:      self.id,
            test_id: self.test_id,
        }).await;
    }
}

#[derive(Debug)]
struct Output {
    addr: IpAddr,
    port: u16,
    sent: u32,
    lost: u32,
    rtt:  Summary,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self  { sent, lost, rtt: Summary { min, max, avg, jit, .. }, .. } = self;
        let good = sent - lost;
        write!(f, "{}/{} min rtt {:.2?}, max {:.2?}, avg {:.2?}, jitter {:.2?}", good, sent, min, max, avg, jit)
    }
}
