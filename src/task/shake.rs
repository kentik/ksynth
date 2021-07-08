use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use webpki::DNSNameRef;
use synapi::tasks::ShakeConfig;
use crate::export::{record, Envoy};
use crate::status::Active;
use super::{Network, Resolver, Shaker, Task};
use super::tls::Identity;

pub struct Shake {
    task:     u64,
    test:     u64,
    target:   Arc<String>,
    network:  Network,
    port:     u16,
    period:   Duration,
    expiry:   Duration,
    envoy:    Envoy,
    shaker:   Arc<Shaker>,
    resolver: Resolver,
    active:   Arc<Active>,
}

impl Shake {
    pub fn new(task: Task, cfg: ShakeConfig, shaker: Arc<Shaker>) -> Self {
        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   Arc::new(cfg.target),
            port:     cfg.port,
            period:   cfg.period.into(),
            expiry:   cfg.expiry.into(),
            envoy:    task.envoy,
            shaker:   shaker,
            resolver: task.resolver,
            active:   task.active,
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

            sleep(self.period).await;
        }
    }

    async fn shake(&self) -> Result<Output> {
        let _guard = self.active.shake();

        let time = Instant::now();
        let addr = self.resolver.lookup(&self.target, self.network).await?;

        let name = DNSNameRef::try_from_ascii_str(&self.target)?;
        let addr = SocketAddr::new(addr, self.port);

        let c = self.shaker.shake(name, addr).await?;

        Ok(Output {
            addr:   addr.ip(),
            port:   addr.port(),
            server: c.server,
            time:   time.elapsed(),
        })
    }

    async fn success(&self, out: Output) -> Result<()> {
        debug!("{}: {}", self.task, out);

        self.envoy.export(record::Shake {
            task:   self.task,
            test:   self.test,
            target: self.target.clone(),
            addr:   out.addr,
            port:   out.port,
            server: out.server,
            time:   out.time,
        }).await;

        self.active.success();

        Ok(())
    }

    async fn failure(&self, err: Error) {
        warn!("{}: {}", self.task, err);
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

#[derive(Debug)]
struct Output {
    addr:   IpAddr,
    port:   u16,
    server: Identity,
    time:   Duration,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:0.2?}", self.time)
    }
}
