use std::convert::TryFrom;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error, Result};
use futures::TryStreamExt;
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use netdiag::{self, Pinger};
use synapi::tasks::PingConfig;
use crate::export::{record, Envoy};
use crate::net::{Network, Resolver};
use crate::stats::{summarize, Summary};
use crate::status::Active;
use super::{Expiry, Task};

pub struct Ping {
    task:     u64,
    test:     u64,
    network:  Network,
    target:   Arc<String>,
    period:   Duration,
    count:    usize,
    delay:    Duration,
    expiry:   Expiry,
    envoy:    Envoy,
    pinger:   Arc<Pinger>,
    resolver: Resolver,
    active:   Arc<Active>,
}

impl Ping {
    pub fn new(task: Task, cfg: PingConfig, pinger: Arc<Pinger>) -> Self {
        let count  = cfg.count.into();
        let expiry = Expiry::new(cfg.expiry.into(), count);

        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   Arc::new(cfg.target),
            period:   cfg.period.into(),
            count:    count,
            delay:    cfg.delay.into(),
            expiry:   expiry,
            envoy:    task.envoy,
            pinger:   pinger,
            resolver: task.resolver,
            active:   task.active,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let result = self.ping();

            match timeout(self.expiry.task, result).await {
                Ok(Ok(rtt)) => self.success(rtt).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            sleep(self.period).await;
        }
    }

    async fn ping(&self) -> Result<Output> {
        let _guard = self.active.ping();

        let addr = self.resolver.lookup(&self.target, self.network).await?;
        let rtt  = ping(&self, addr).await?;

        let sent = rtt.len();
        let rtt  = rtt.into_iter().flatten().collect::<Vec<_>>();
        let lost = sent - rtt.len();

        Ok(Output {
            addr:   addr,
            sent:   u32::try_from(sent)?,
            lost:   u32::try_from(lost)?,
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
        self.active.success();
    }

    async fn failure(&self, err: Error) {
        warn!("{}: error: {}", self.task, err);
        self.envoy.export(record::Error {
            task:  self.task,
            test:  self.test,
            cause: err.to_string(),
        }).await;
        self.active.failure();
    }

    async fn timeout(&self) {
        warn!("{}: timeout", self.task);
        self.envoy.export(record::Timeout {
            task: self.task,
            test: self.test,
        }).await;
        self.active.timeout();
    }
}

async fn ping(ping: &Ping, addr: IpAddr) -> Result<Vec<Option<Duration>>> {
    let pinger = &ping.pinger;
    let delay  = ping.delay;

    let ping = netdiag::Ping {
        addr:   addr,
        count:  ping.count,
        expiry: ping.expiry.probe,
    };

    pinger.ping(&ping).and_then(|rtt| async move {
        sleep(delay).await;
        Ok(rtt)
    }).try_collect().await
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
