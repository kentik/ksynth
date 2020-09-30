use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use bytes::Bytes;
use log::{debug, warn};
use reqwest::{Client, Method, Request, RequestBuilder, StatusCode, Url};
use reqwest::header::HOST;
use tokio::time::{delay_for, timeout};
use netdiag::Bind;
use synapi::tasks::FetchConfig;
use crate::export::{record, Envoy};
use super::{Network, Resolver, Task};

pub struct Fetch {
    task:     u64,
    test:     u64,
    network:  Network,
    target:   String,
    period:   Duration,
    expiry:   Duration,
    envoy:    Envoy,
    client:   Arc<Fetcher>,
    resolver: Resolver,
}

impl Fetch {
    pub fn new(task: Task, cfg: FetchConfig, client: Arc<Fetcher>) -> Self {
        Self {
            task:     task.task,
            test:     task.test,
            network:  task.network,
            target:   cfg.target,
            period:   Duration::from_secs(cfg.period),
            expiry:   Duration::from_millis(cfg.expiry),
            envoy:    task.envoy,
            client:   client,
            resolver: task.resolver,
        }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let target = Url::parse(&self.target)?;
            let result = self.fetch(target);

            match timeout(self.expiry, result).await {
                Ok(Ok(stats)) => self.success(stats).await,
                Ok(Err(e))    => self.failure(e).await,
                Err(_)        => self.timeout().await,
            }

            delay_for(self.period).await;
        }
    }

    async fn fetch(&self, mut target: Url) -> Result<Output> {
        let method   = Method::GET;
        let start    = Instant::now();
        let mut host = None;

        if let Some(name) = target.domain().map(str::to_owned) {
            let addr = self.resolver.lookup(&name, self.network).await?;
            target.set_ip_host(addr).expect("IP address");
            host = Some(name);
        }

        let req = self.client.request(method, target, host).build()?;
        self.client.execute(start, req).await
    }

    async fn success(&self, out: Output) {
        debug!("{}: {}", self.task, out);
        self.envoy.export(record::Fetch {
            task:    self.task,
            test:    self.test,
            addr:    out.addr,
            status:  out.status.as_u16(),
            rtt:     out.rtt,
            size:    out.body.len(),
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

#[derive(Clone)]
pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub fn new(bind: &Bind) -> Result<Self> {
        let mut client = Client::builder();
        client = client.timeout(Duration::from_secs(10));
        client = client.local_address(bind.sa4().ip());
        client = client.pool_max_idle_per_host(0);
        let client = client.build()?;

        Ok(Self { client })
    }

    pub fn request(&self, method: Method, url: Url, host: Option<String>) -> RequestBuilder {
        match host {
            Some(addr) => self.client.request(method, url).header(HOST, addr),
            None       => self.client.request(method, url)
        }
    }

    pub async fn execute(&self, start: Instant, req: Request) -> Result<Output> {
        let res = self.client.execute(req).await?;

        let addr = match res.remote_addr() {
            Some(sa) => sa.ip(),
            None     => IpAddr::V4(0.into()),
        };

        let status = res.status();
        let body   = res.bytes().await?;
        let time   = Instant::now();
        let rtt    = time.saturating_duration_since(start);

        Ok(Output { addr, status, rtt, body })
    }
}

#[derive(Debug)]
pub struct Output {
    addr:   IpAddr,
    status: StatusCode,
    rtt:    Duration,
    body:   Bytes,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { rtt, status, body, .. } = self;
        let status = status.as_u16();
        let size   = body.len();
        write!(f, "rtt: {:.2?}, status: {}, bytes: {}", rtt, status, size)
    }
}
