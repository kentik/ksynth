use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error, Result};
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use netdiag::{self, Pinger};
use synapi::tasks::PingConfig;
use crate::export::{record, Envoy};
use crate::stats::{summarize, Summary};
use super::{Network, Resolver, Task};

pub struct Ping {
    task:     u64,
    test:     u64,
    network:  Network,
    target:   Arc<String>,
    period:   Duration,
    count:    usize,
    expiry:   Duration,
    envoy:    Envoy,
    pinger:   Arc<Pinger>,
    resolver: Resolver,
}

impl Ping {
    pub fn new(task: Task, cfg: PingConfig, pinger: Arc<Pinger>) -> Self {
        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   Arc::new(cfg.target),
            period:   Duration::from_secs(cfg.period),
            count:    cfg.count as usize,
            expiry:   Duration::from_millis(cfg.expiry),
            envoy:    task.envoy,
            pinger:   pinger,
            resolver: task.resolver,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let result = self.ping(self.count);

            match timeout(self.expiry, result).await {
                Ok(Ok(rtt)) => self.success(rtt).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            sleep(self.period).await;
        }
    }

    async fn ping(&self, count: usize) -> Result<Output> {
        let addr = self.resolver.lookup(&self.target, self.network).await?;

        let ping = netdiag::Ping {
            addr:   addr,
            count:  count,
            expiry: Duration::from_millis(250),
        };

        let rtt  = self.pinger.ping(ping).await?;
        let sent = rtt.len() as u32;
        let rtt  = rtt.into_iter().flatten().collect::<Vec<_>>();
        let lost = sent - rtt.len() as u32;

        Ok(Output {
            addr:   addr,
            sent:   sent,
            lost:   lost,
            rtt:    summarize(&rtt).unwrap_or_default(),
            result: rtt,
        })
    }

    async fn success(&self, out: Output) {
        debug!("{}: {}", self.task, out);
        self.envoy.export(record::Ping {
            task:   self.task,
            test:   self.test,
            target: self.target.clone(),
            addr:   out.addr,
            sent:   out.sent,
            lost:   out.lost,
            rtt:    out.rtt,
            result: out.result,
        }).await;
    }

    async fn failure(&self, err: Error) {
        warn!("{}: error: {}", self.task, err);
        self.envoy.export(record::Error {
            task:  self.task,
            test:  self.test,
            cause: err.to_string(),
        }).await;
    }

    async fn timeout(&self) {
        warn!("{}: timeout", self.task);
        self.envoy.export(record::Timeout {
            task: self.task,
            test: self.test,
        }).await;
    }
}

#[derive(Debug)]
struct Output {
    addr:   IpAddr,
    sent:   u32,
    lost:   u32,
    rtt:    Summary,
    result: Vec<Duration>,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self  { sent, lost, rtt: Summary { min, max, avg, jit, .. }, .. } = self;
        let good = sent - lost;
        write!(f, "{}/{} min rtt {:.2?}, max {:.2?}, avg {:.2?}, jitter {:.2?}", good, sent, min, max, avg, jit)
    }
}
