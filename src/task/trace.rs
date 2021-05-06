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
use crate::status::Active;
use super::{Expiry, Network, Resolver, Task};

pub struct Trace {
    task:     u64,
    test:     u64,
    protocol: Protocol,
    target:   Arc<String>,
    network:  Network,
    period:   Duration,
    count:    usize,
    limit:    usize,
    expiry:   Expiry,
    envoy:    Envoy,
    tracer:   Arc<Tracer>,
    resolver: Resolver,
    active:   Arc<Active>,
}

impl Trace {
    pub fn new(task: Task, cfg: TraceConfig, tracer: Arc<Tracer>) -> Self {
        let TraceConfig { protocol, port, .. } = cfg;

        let protocol = match &*protocol {
            "ICMP"            => Protocol::ICMP,
            "TCP" if port > 0 => Protocol::TCP(port),
            "UDP" if port > 0 => Protocol::UDP(port),
            _                 => Protocol::default(),
        };

        let count  = usize::from(cfg.count);
        let limit  = usize::from(cfg.limit);
        let expiry = Expiry::new(cfg.expiry.into(), count * limit);

        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            protocol: protocol,
            target:   Arc::new(cfg.target),
            period:   cfg.period.into(),
            count:    count,
            limit:    limit,
            expiry:   expiry,
            envoy:    task.envoy,
            tracer:   tracer,
            resolver: task.resolver,
            active:   task.active,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let result = self.trace();

            match timeout(self.expiry.task, result).await {
                Ok(Ok(stats)) => self.success(stats).await?,
                Ok(Err(e))    => self.failure(e).await,
                Err(_)        => self.timeout().await,
            }

            sleep(self.period).await;
        }
    }

    async fn trace(&self) -> Result<Output> {
        let _guard = self.active.trace();

        let time = Instant::now();
        let addr = self.resolver.lookup(&self.target, self.network).await?;

        let route = self.tracer.route(netdiag::Trace {
            proto:  self.protocol,
            addr:   addr,
            probes: self.count,
            limit:  self.limit,
            expiry: self.expiry.probe,
        }).await?;

        Ok(Output {
            addr:  addr,
            route: route,
            time:  time.elapsed(),
        })
    }

    async fn success(&self, out: Output) -> Result<()> {
        debug!("{}: {}", self.task, out);

        let hops = out.route.into_iter().enumerate().map(|(hop, nodes)| {
            let mut map = HashMap::<IpAddr, Vec<u64>>::new();

            for node in nodes {
                if let Node::Node(_, addr, rtt, _) = node {
                    let rtt = rtt.as_micros() as u64;
                    map.entry(addr).or_default().push(rtt);
                }
            }

            Hop { hop: hop + 1, nodes: map }
        }).collect::<Vec<_>>();

        let route = serde_json::to_string(&hops)?;

        self.envoy.export(record::Trace {
            task:   self.task,
            test:   self.test,
            target: self.target.clone(),
            addr:   out.addr,
            hops:   hops,
            route:  route,
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
