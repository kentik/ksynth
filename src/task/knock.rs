use std::convert::TryFrom;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Error,Result}  ;
use futures::TryStreamExt;
use tracing::{debug, info_span, warn, Instrument};
use tokio::time::{sleep, timeout};
use netdiag::{self, Knocker};
use synapi::tasks::KnockConfig;
use crate::export::{record, Envoy};
use crate::net::{Network, Resolver};
use crate::stats::{summarize, Summary};
use crate::status::Active;
use super::{Expiry, Task};

pub struct Knock {
    task:     u64,
    test:     u64,
    network:  Network,
    target:   Arc<String>,
    port:     u16,
    period:   Duration,
    count:    usize,
    delay:    Duration,
    expiry:   Expiry,
    envoy:    Envoy,
    knocker:  Arc<Knocker>,
    resolver: Resolver,
    active:   Arc<Active>,
}

impl Knock {
    pub fn new(task: Task, cfg: KnockConfig, knocker: Arc<Knocker>) -> Self {
        let count  = cfg.count.into();
        let expiry = Expiry::new(cfg.expiry.into(), count);

        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   Arc::new(cfg.target),
            port:     cfg.port,
            period:   cfg.period.into(),
            count:    count,
            delay:    cfg.delay.into(),
            expiry:   expiry,
            envoy:    task.envoy,
            knocker:  knocker,
            resolver: task.resolver,
            active:   task.active,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            let Self { task, test, target, port, .. } = &self;

            let span = info_span!("knock", task, test);

            async {
                let _guard = self.active.knock();
                let result = self.knock(target, *port);

                match timeout(self.expiry.task, result).await {
                    Ok(Ok(rtt)) => self.success(rtt).await,
                    Ok(Err(e))  => self.failure(e).await,
                    Err(_)      => self.timeout().await,
                };
            }.instrument(span).await;

            sleep(self.period).await;
        }
    }

    async fn knock(&self, target: &str, port: u16) -> Result<Output> {
        let addr = self.resolver.lookup(target, self.network).await?;

        debug!("target {target}:{port} ({addr})");

        let rtt  = knock(&self, addr, port).await?;
        let sent = rtt.len();
        let rtt  = rtt.into_iter().flatten().collect::<Vec<_>>();
        let lost = sent - rtt.len();

        Ok(Output {
            addr:   addr,
            port:   port,
            sent:   u32::try_from(sent)?,
            lost:   u32::try_from(lost)?,
            rtt:    summarize(&rtt).unwrap_or_default(),
            result: rtt,
        })
    }

    async fn success(&self, out: Output) {
        debug!("{out}");
        self.envoy.export(record::Knock {
            target: self.target.clone(),
            task:   self.task,
            test:   self.test,
            addr:   out.addr,
            port:   out.port,
            sent:   out.sent,
            lost:   out.lost,
            rtt:    out.rtt,
            result: out.result,
        }).await;
        self.active.success();
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

async fn knock(knock: &Knock, addr: IpAddr, port: u16) -> Result<Vec<Option<Duration>>> {
    let knocker = &knock.knocker;
    let delay   = knock.delay;

    let knock = netdiag::Knock {
        addr:   addr,
        port:   port,
        count:  knock.count,
        expiry: knock.expiry.probe,
    };

    knocker.knock(&knock).await?.and_then(|rtt| async move {
        sleep(delay).await;
        Ok(rtt)
    }).try_collect().await
}

#[derive(Debug)]
struct Output {
    addr:   IpAddr,
    port:   u16,
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
