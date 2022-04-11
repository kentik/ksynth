use std::convert::TryFrom;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use tracing::{debug, info_span, warn, Instrument};
use rustls::ServerName;
use tokio::time::{sleep, timeout};
use synapi::tasks::ShakeConfig;
use crate::export::{record, Envoy};
use crate::net::{Network, Resolver};
use crate::net::tls::{Identity, Shaker};
use crate::status::Active;
use super::Task;

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
            let task = self.task;
            let test = self.test;

            let span = info_span!("shake", task, test);

            async {
                let _guard = self.active.shake();
                let result = self.shake(&self.target);

                match timeout(self.expiry, result).await {
                    Ok(Ok(out)) => self.success(out).await?,
                    Ok(Err(e))  => self.failure(e).await,
                    Err(_)      => self.timeout().await,
                }

                Result::<_, Error>::Ok(())
            }.instrument(span).await?;

            sleep(self.period).await;
        }
    }

    async fn shake(&self, target: &str) -> Result<Output> {
        let time = Instant::now();
        let addr = self.resolver.lookup(target, self.network).await?;

        debug!("target {target} ({addr})");

        let name = ServerName::try_from(self.target.as_str())?;
        let addr = SocketAddr::new(addr, self.port);

        let c = self.shaker.shake(&name, addr).await?;

        Ok(Output {
            addr:   addr.ip(),
            port:   addr.port(),
            server: c.server,
            time:   time.elapsed(),
        })
    }

    async fn success(&self, out: Output) -> Result<()> {
        debug!("{out}");

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
        warn!(error = &*err.to_string());
        self.envoy.export(record::Error {
            task:  self.task,
            test:  self.test,
            cause: err.to_string(),
        }).await;
        self.active.failure();
    }

    async fn timeout(&self) {
        warn!("timeout");
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
