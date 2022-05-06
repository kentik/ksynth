use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use bytes::Bytes;
use anyhow::{Error, Result};
use hyper::{Body, Method, StatusCode};
use hyper::body::HttpBody;
use hyper::header::HeaderMap;
use tracing::{debug, info_span, warn, Instrument};
use tokio::time::{sleep, timeout};
use synapi::tasks::FetchConfig;
use crate::cfg::Config;
use crate::export::{record, Envoy};
use crate::net::Network;
use crate::net::http::{HttpClient, Request};
use crate::net::tls::Identity;
use crate::status::Active;
use super::Task;

pub struct Fetch {
    task:    u64,
    test:    u64,
    network: Network,
    target:  Arc<String>,
    method:  Method,
    headers: Option<HeaderMap>,
    body:    Option<Bytes>,
    verify:  bool,
    period:  Duration,
    expiry:  Duration,
    envoy:   Envoy,
    client:  Arc<Fetcher>,
    active:  Arc<Active>,
}

impl Fetch {
    pub fn new(task: Task, cfg: FetchConfig, client: Arc<Fetcher>) -> Result<Self> {
        let method  = cfg.method.parse().unwrap_or(Method::GET);
        let headers = cfg.headers.map(|map| {
            let map = map.iter().map(|(name, value)| {
                let name  = name.parse()?;
                let value = value.parse()?;
                Ok((name, value))
            }).collect::<Result<_>>()?;
            Result::<_, Error>::Ok(map)
        }).transpose()?;

        Ok(Self {
            task:    task.task,
            test:    task.test,
            network: task.network,
            target:  Arc::new(cfg.target),
            method:  method,
            headers: headers,
            body:    cfg.body.map(Bytes::from),
            verify:  !cfg.insecure,
            period:  cfg.period.into(),
            expiry:  cfg.expiry.into(),
            envoy:   task.envoy,
            client:  client,
            active:  task.active,
        })
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            let task = self.task;
            let test = self.test;

            let span = info_span!("fetch", task, test);

            async {
                let _guard = self.active.fetch();
                let result = self.fetch(&self.target);

                match timeout(self.expiry, result).await {
                    Ok(Ok(stats)) => self.success(stats).await,
                    Ok(Err(e))    => self.failure(e).await,
                    Err(_)        => self.timeout().await,
                }
            }.instrument(span).await;

            sleep(self.period).await;
        }
    }

    async fn fetch(&self, target: &str) -> Result<Output> {
        debug!("target {}", target);

        let network = self.network;
        let method  = self.method.clone();
        let body    = self.body.clone();
        let start   = Instant::now();

        let mut req = Request::new(network, method, target.parse()?)?;
        *req.body() = body.map(Body::from).unwrap_or_else(Body::empty);

        if let Some(headers) = self.headers.as_ref().cloned() {
            req.headers().extend(headers);
        }

        let output = self.client.execute(start, req).await?;

        if let Identity::Error(e) = &output.server {
            if self.verify {
                return Err(e.clone().into());
            }
        }

        Ok(output)
    }

    async fn success(&self, out: Output) {
        debug!("{out}");
        self.envoy.export(record::Fetch {
            task:    self.task,
            test:    self.test,
            target:  self.target.clone(),
            addr:    out.addr,
            server:  out.server,
            status:  out.status.as_u16(),
            dns:     out.dns,
            tcp:     out.tcp,
            tls:     out.tls,
            rtt:     out.rtt,
            size:    out.bytes,
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

#[derive(Clone)]
pub struct Fetcher {
    client: HttpClient,
}

impl Fetcher {
    pub fn new(cfg: &Config) -> Result<Self> {
        let Config { bind, resolver, roots, .. } = cfg.clone();
        let client = HttpClient::new(bind, resolver, roots)?;
        Ok(Self { client })
    }

    pub async fn execute(&self, start: Instant, req: Request) -> Result<Output> {
        let mut res = self.client.request(req).await?;

        let addr = res.peer.addr.ip();
        let body = &mut res.body;

        let mut bytes: usize = 0;
        while let Some(chunk) = body.data().await {
            bytes += chunk?.len();
        }

        let status = res.head.status;
        let time   = Instant::now();
        let rtt    = time.saturating_duration_since(start);

        let server = res.peer.server;
        let times  = res.times;
        let dns    = times.dns;
        let tcp    = times.tcp;
        let tls    = times.tls.unwrap_or_default();

        Ok(Output { addr, server, status, dns, tcp, tls, rtt, bytes })
    }
}

#[derive(Debug)]
pub struct Output {
    addr:   IpAddr,
    server: Identity,
    status: StatusCode,
    dns:    Duration,
    tcp:    Duration,
    tls:    Duration,
    rtt:    Duration,
    bytes:  usize,
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { rtt, status, bytes, .. } = self;
        let status = status.as_u16();
        write!(f, "rtt: {:.2?}, status: {}, bytes: {}", rtt, status, bytes)
    }
}
