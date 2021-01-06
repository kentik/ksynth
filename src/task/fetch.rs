use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use hyper::{Body, Method, Request, StatusCode, Uri};
use hyper::body::HttpBody;
use hyper::client::connect::HttpInfo;
use log::{debug, warn};
use tokio::time::{sleep, timeout};
use synapi::tasks::FetchConfig;
use crate::export::{record, Envoy};
use super::{Config, Task, http::{Expiry, HttpClient}};

pub struct Fetch {
    task:   u64,
    test:   u64,
    target: String,
    period: Duration,
    expiry: Duration,
    envoy:  Envoy,
    client: Arc<Fetcher>,
}

impl Fetch {
    pub fn new(task: Task, cfg: FetchConfig, client: Arc<Fetcher>) -> Self {
        Self {
            task:   task.task,
            test:   task.test,
            target: cfg.target,
            period: Duration::from_secs(cfg.period),
            expiry: Duration::from_millis(cfg.expiry),
            envoy:  task.envoy,
            client: client,
        }
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
        let method   = Method::GET;
        let start    = Instant::now();

        let req = self.client.request(method, target)?;
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
            size:    out.bytes,
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

    pub fn request(&self, method: Method, url: Uri) -> Result<Request<Body>> {
        Ok(Request::builder().method(method).uri(url).body(Body::empty())?)
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

        Ok(Output { addr, status, rtt, bytes })
    }
}

#[derive(Debug)]
pub struct Output {
    addr:   IpAddr,
    status: StatusCode,
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
