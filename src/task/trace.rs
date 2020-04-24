use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use log::{debug, warn};
use tokio::time::{delay_for, timeout};
use netdiag::{self, Node, Tracer};
use synapi::tasks::TraceConfig;
use crate::export::{record, Envoy};
use super::resolve;

pub struct Trace {
    id:     u64,
    target: String,
    period: Duration,
    limit:  usize,
    expiry: Duration,
    envoy:  Envoy,
    tracer: Arc<Tracer>,
}

impl Trace {
    pub fn new(id: u64, cfg: TraceConfig, envoy: Envoy, tracer: Arc<Tracer>) -> Self {
        let TraceConfig { target, period, limit, expiry } = cfg;

        let period = Duration::from_secs(period);
        let limit  = limit as usize;
        let expiry = Duration::from_millis(expiry);

        Self { id, target, period, limit, expiry, envoy, tracer }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: target {}", self.id, self.target);

            let result = self.trace();

            match timeout(self.expiry, result).await {
                Ok(Ok(stats)) => self.success(stats).await,
                Ok(Err(e))    => self.failure(e).await,
                Err(_)        => self.timeout().await,
            }

            delay_for(self.period).await;
        }
    }

    async fn trace(&self) -> Result<Stats> {
        let time = Instant::now();
        let addr = resolve(&self.target).await?;

        let route = self.tracer.route(netdiag::Trace {
            addr:   addr,
            probes: 3,
            limit:  self.limit,
            expiry: Duration::from_millis(250),
        }).await?;

        Ok(Stats {
            addr:  IpAddr::V4(addr),
            route: route,
            time:  time.elapsed(),
        })
    }

    async fn success(&self, stats: Stats) {
        debug!("{}: {}", self.id, stats);
        self.envoy.export(record::Trace {
            id:    self.id,
            addr:  stats.addr,
            route: stats.route,
            time:  stats.time,
        }).await;
    }

    async fn failure(&self, err: Error) {
        warn!("{}: {}", self.id, err);
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

#[derive(Debug)]
struct Stats {
    addr:  IpAddr,
    route: Vec<Vec<Node>>,
    time:  Duration,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { route, time, .. } = self;
        write!(f, "{} hops in {:0.2?}", route.len(), time)
    }
}
