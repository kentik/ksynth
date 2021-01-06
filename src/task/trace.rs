use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use netdiag::{self, Node, Protocol, Tracer};
use synapi::tasks::TraceConfig;
use crate::export::{record, Hop, Envoy};
use super::{Network, Resolver, Task};

pub struct Trace {
    task:     u64,
    test:     u64,
    protocol: Protocol,
    target:   String,
    network:  Network,
    period:   Duration,
    limit:    usize,
    expiry:   Duration,
    envoy:    Envoy,
    tracer:   Arc<Tracer>,
    resolver: Resolver,
}

impl Trace {
    pub fn new(task: Task, cfg: TraceConfig, tracer: Arc<Tracer>) -> Self {
        let TraceConfig { protocol, port, .. } = cfg;

        let protocol = match &*protocol {
            "TCP" if port > 0 => Protocol::TCP(port),
            "UDP" if port > 0 => Protocol::UDP(port),
            _                 => Protocol::default(),
        };

        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            protocol: protocol,
            target:   cfg.target,
            period:   Duration::from_secs(cfg.period),
            limit:    cfg.limit as usize,
            expiry:   Duration::from_millis(cfg.expiry),
            envoy:    task.envoy,
            tracer:   tracer,
            resolver: task.resolver,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let result = self.trace();

            match timeout(self.expiry, result).await {
                Ok(Ok(stats)) => self.success(stats).await?,
                Ok(Err(e))    => self.failure(e).await,
                Err(_)        => self.timeout().await,
            }

            sleep(self.period).await;
        }
    }

    async fn trace(&self) -> Result<Output> {
        let time = Instant::now();
        let addr = self.resolver.lookup(&self.target, self.network).await?;

        let route = self.tracer.route(netdiag::Trace {
            proto:  self.protocol,
            addr:   addr,
            probes: 3,
            limit:  self.limit,
            expiry: Duration::from_millis(250),
        }).await?;

        Ok(Output {
            addr:  addr,
            route: route,
            time:  time.elapsed(),
        })
    }

    async fn success(&self, out: Output) -> Result<()> {
        debug!("{}: {}", self.task, out);

        let route = out.route.into_iter().enumerate().map(|(hop, nodes)| {
            let mut map = HashMap::<IpAddr, Vec<u64>>::new();

            for node in nodes {
                if let Node::Node(_, addr, rtt, _) = node {
                    let rtt = rtt.as_micros() as u64;
                    map.entry(addr).or_default().push(rtt);
                }
            }

            Hop { hop: hop + 1, nodes: map }
        }).collect::<Vec<_>>();

        let route = serde_json::to_string(&route)?;

        self.envoy.export(record::Trace {
            task:  self.task,
            test:  self.test,
            addr:  out.addr,
            route: route,
            time:  out.time,
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
    addr:  IpAddr,
    route: Vec<Vec<Node>>,
    time:  Duration,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { route, time, .. } = self;
        write!(f, "{} hops in {:0.2?}", route.len(), time)
    }
}
