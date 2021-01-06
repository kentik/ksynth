use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error,Result}  ;
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use netdiag::{self, Knocker};
use synapi::tasks::KnockConfig;
use crate::export::{record, Envoy};
use crate::stats::{summarize, Summary};
use super::{Network, Resolver, Task};

pub struct Knock {
    task:     u64,
    test:     u64,
    network:  Network,
    target:   String,
    port:     u16,
    period:   Duration,
    count:    usize,
    expiry:   Duration,
    envoy:    Envoy,
    knocker:  Arc<Knocker>,
    resolver: Resolver,
}

impl Knock {
    pub fn new(task: Task, cfg: KnockConfig, knocker: Arc<Knocker>) -> Self {
        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   cfg.target,
            port:     cfg.port,
            period:   Duration::from_secs(cfg.period),
            count:    cfg.count as usize,
            expiry:   Duration::from_millis(cfg.expiry),
            envoy:    task.envoy,
            knocker:  knocker,
            resolver: task.resolver,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            let Self { task, test, target, port, .. } = &self;

            debug!("{}: test {}, target {}:{}", task, test, target, port);

            let result = self.knock(self.count);

            match timeout(self.expiry, result).await {
                Ok(Ok(rtt)) => self.success(rtt).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            sleep(self.period).await;
        }
    }

    async fn knock(&self, count: usize) -> Result<Output> {
        let knocker = &self.knocker;

        let addr = self.resolver.lookup(&self.target, self.network).await?;
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
        debug!("{}: {}", self.task, out);
        self.envoy.export(record::Knock {
            task: self.task,
            test: self.test,
            addr: out.addr,
            port: out.port,
            sent: out.sent,
            lost: out.lost,
            rtt:  out.rtt,
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
