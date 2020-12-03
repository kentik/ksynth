use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use log::{debug, warn};
use tokio::time::{delay_for, timeout};
use webpki::DNSNameRef;
use synapi::tasks::ShakeConfig;
use crate::export::{record, Envoy};
use super::{Network, Resolver, Shaker, Task};

pub struct Shake {
    task:     u64,
    test:     u64,
    target:   String,
    network:  Network,
    port:     u16,
    period:   Duration,
    expiry:   Duration,
    envoy:    Envoy,
    shaker:   Arc<Shaker>,
    resolver: Resolver,
}

impl Shake {
    pub fn new(task: Task, cfg: ShakeConfig, shaker: Arc<Shaker>) -> Self {
        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   cfg.target,
            port:     cfg.port,
            period:   Duration::from_secs(cfg.period),
            expiry:   Duration::from_millis(cfg.expiry),
            envoy:    task.envoy,
            shaker:   shaker,
            resolver: task.resolver,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let result = self.shake();

            match timeout(self.expiry, result).await {
                Ok(Ok(out)) => self.success(out).await?,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            }

            delay_for(self.period).await;
        }
    }

    async fn shake(&self) -> Result<Output> {
        let time = Instant::now();
        let addr = self.resolver.lookup(&self.target, self.network).await?;

        let name = DNSNameRef::try_from_ascii_str(&self.target)?;
        let addr = SocketAddr::new(addr, self.port);

        self.shaker.shake(name, addr).await?;

        Ok(Output {
            addr: addr.ip(),
            port: addr.port(),
            time: time.elapsed(),
        })
    }

    async fn success(&self, out: Output) -> Result<()> {
        debug!("{}: {}", self.task, out);

        self.envoy.export(record::Shake {
            task: self.task,
            test: self.test,
            addr: out.addr,
            port: out.port,
            time: out.time,
        }).await;

        Ok(())
    }

    async fn failure(&self, err: Error) {
        warn!("{}: {}", self.task, err);
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
    time: Duration,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:0.2?}", self.time)
    }
}