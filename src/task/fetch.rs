use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use bytes::Bytes;
use anyhow::{Error, Result};
use hyper::{Body, Method, Request, StatusCode, Uri};
use hyper::body::HttpBody;
use hyper::client::connect::HttpInfo;
use hyper::header::HeaderMap;
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use synapi::tasks::FetchConfig;
use crate::export::{record, Envoy};
use crate::status::Active;
use super::{Config, Task, http::{Expiry, HttpClient, Times}};
use super::tls::Identity;

pub struct Fetch {
    task:    u64,
    test:    u64,
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
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let target = self.target.parse()?;
            let result = self.fetch(target);

            match timeout(self.expiry, result).await {
                Ok(Ok(stats)) => self.success(stats).await,
                Ok(Err(e))    => self.failure(e).await,
                Err(_)        => self.timeout().await,
            }

            sleep(self.period).await;
        }
    }

    async fn fetch(&self, target: Uri) -> Result<Output> {
        let _guard = self.active.fetch();

        let method = self.method.clone();
        let body   = self.body.clone().map(Body::from).unwrap_or_else(Body::empty);
        let start  = Instant::now();

        let mut req = self.client.request(method, target, body)?;

        if let Some(headers) = self.headers.as_ref().cloned() {
            req.headers_mut().extend(headers);
        }

        let output = self.client.execute(start, req).await?;

        if let Identity::Error(e) = output.server {
            if self.verify {
                return Err(e.into());
            }
        }

        Ok(output)
    }

    async fn success(&self, out: Output) {
        debug!("{}: {}", self.task, out);
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

#[derive(Clone)]
pub struct Fetcher {
    client: HttpClient,
}

impl Fetcher {
    pub fn new(cfg: &Config) -> Result<Self> {
        let expiry = Expiry {
            connect: Duration::from_secs(10),
            request: Duration::from_secs(60),
        };
        let client = HttpClient::new(cfg, expiry)?;
        Ok(Self { client })
    }

    pub fn request(&self, method: Method, url: Uri, body: Body) -> Result<Request<Body>> {
        Ok(Request::builder().method(method).uri(url).body(body)?)
    }

    pub async fn execute(&self, start: Instant, req: Request<Body>) -> Result<Output> {
        let mut res = self.client.request(req).await?;

        let addr = match res.extensions().get::<HttpInfo>() {
            Some(info) => info.remote_addr().ip(),
            None       => IpAddr::V4(0.into()),
        };

        let mut bytes: usize = 0;
        while let Some(chunk) = res.data().await {
            bytes += chunk?.len();
        }

        let status = res.status();
        let time   = Instant::now();
        let rtt    = time.saturating_duration_since(start);

        let exts = res.extensions();

        let times  = exts.get::<Times>().cloned().unwrap_or_default();
        let server = exts.get::<Identity>().cloned().unwrap_or_default();
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
