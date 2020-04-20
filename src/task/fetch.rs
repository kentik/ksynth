use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use bytes::Bytes;
use reqwest::{Client, StatusCode};
use log::{debug, warn};
use tokio::time::delay_for;

pub struct Fetch {
    id:     u64,
    target: String,
    period: Duration,
    client: Arc<Fetcher>,
}

impl Fetch {
    pub fn new(id: u64, target: &str, client: Arc<Fetcher>) -> Self {
        let target = format!("https://{}", target);
        let period = Duration::from_secs(30);
        Self { id, target, period, client }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: target {}", self.id, self.target);

            match self.client.get(&self.target).await {
                Ok(stats) => self.report(stats),
                Err(e)    => warn!("{}", e),
            }

            delay_for(self.period).await;
        }
    }

    fn report(&self, stats: Stats) {
        let rtt  = stats.rtt;
        let code = stats.status.as_u16();
        let size = stats.body.len();
        debug!("{}: rtt: {:.2?}, status: {}, bytes: {}", self.id, rtt, code, size);
    }
}

#[derive(Debug)]
pub struct Stats {
    status: StatusCode,
    rtt:    Duration,
    body:   Bytes,
}

#[derive(Clone)]
pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let mut client = Client::builder();
        client = client.timeout(Duration::from_secs(2));
        let client = client.build()?;

        Ok(Self { client })
    }

    pub async fn get(&self, url: &str) -> Result<Stats> {
        let time = Instant::now();
        let res = self.client.get(url).send().await?;

        let status = res.status();
        let body   = res.bytes().await?;
        let rtt    = time.elapsed();

        Ok(Stats { status, rtt, body })
    }
}
