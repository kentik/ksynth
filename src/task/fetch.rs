use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use bytes::Bytes;
use reqwest::{Client, StatusCode};
use log::{debug, warn};
use tokio::time::{delay_for, timeout};
use synapi::tasks::FetchConfig;
use crate::export::{record, Envoy};

pub struct Fetch {
    id:      u64,
    test_id: u64,
    target:  String,
    period:  Duration,
    expiry:  Duration,
    envoy:   Envoy,
    client:  Arc<Fetcher>,
}

impl Fetch {
    pub fn new(id: u64, test_id: u64, cfg: FetchConfig, envoy: Envoy, client: Arc<Fetcher>) -> Self {
        let FetchConfig { target, period, expiry } = cfg;

        let period = Duration::from_secs(period);
        let expiry = Duration::from_millis(expiry);

        Self { id, test_id, target, period, expiry, envoy, client }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.id, self.test_id, self.target);

            let result = self.client.get(&self.target);

            match timeout(self.expiry, result).await {
                Ok(Ok(stats)) => self.success(stats).await,
                Ok(Err(e))    => self.failure(e).await,
                Err(_)        => self.timeout().await,
            }

            delay_for(self.period).await;
        }
    }

    async fn success(&self, out: Output) {
        debug!("{}: {}", self.id, out);
        self.envoy.export(record::Fetch {
            id:      self.id,
            test_id: self.test_id, 
            addr:    out.addr,
            status:  out.status.as_u16(),
            rtt:     out.rtt,
            size:    out.body.len(),
        }).await;
    }

    async fn failure(&self, err: Error) {
        warn!("{}: error: {}", self.id, err);
        self.envoy.export(record::Error {
            id:    self.id,
            test_id: self.test_id,   
            cause: err.to_string(),
        }).await;
    }

    async fn timeout(&self) {
        warn!("{}: timeout", self.id);
        self.envoy.export(record::Timeout {
            id: self.id,
            test_id: self.test_id,    
        }).await;
    }
}

#[derive(Clone)]
pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let mut client = Client::builder();
        client = client.timeout(Duration::from_secs(10));
        let client = client.build()?;

        Ok(Self { client })
    }

    pub async fn get(&self, url: &str) -> Result<Output> {
        let sent = Instant::now();
        let res = self.client.get(url).send().await?;

        let addr = match res.remote_addr() {
            Some(sa) => sa.ip(),
            None     => IpAddr::V4(0.into()),
        };

        let status = res.status();
        let body   = res.bytes().await?;
        let time   = Instant::now();
        let rtt    = time.saturating_duration_since(sent);

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
