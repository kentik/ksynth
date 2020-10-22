use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Error, Result};
use log::{debug, warn};
use tokio::net::UdpSocket;
use tokio::time::{delay_for, timeout};
use trust_dns_client::client::{AsyncClient, ClientHandle};
use trust_dns_client::op::{DnsResponse, ResponseCode};
use trust_dns_client::rr::{DNSClass, Name, RecordType, RData};
use trust_dns_client::udp::UdpClientStream;
use trust_dns_proto::udp::UdpResponse;
use synapi::tasks::QueryConfig;
use crate::export::{record, Envoy};
use super::Task;

pub struct Query {
    task:   u64,
    test:   u64,
    target: Name,
    period: Duration,
    expiry: Duration,
    record: RecordType,
    envoy:  Envoy,
    client: AsyncClient<UdpResponse>,
}

impl Query {
    pub async fn new(task: Task, cfg: QueryConfig) -> Result<Self> {
        let server = SocketAddr::from((&cfg.server.parse()?, cfg.port));
        let stream = UdpClientStream::<UdpSocket>::new(server);

        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self {
            task:   task.task,
            test:   task.test,
            target: cfg.target.parse()?,
            period: Duration::from_secs(cfg.period),
            expiry: Duration::from_millis(cfg.expiry),
            record: cfg.record.parse()?,
            envoy:  task.envoy,
            client: client,
        })
    }

    pub async fn exec(mut self) -> Result<()> {
        loop {
            debug!("{}: test {}, target {}", self.task, self.test, self.target);

            let expiry = self.expiry;
            let result = self.query();

            match timeout(expiry, result).await {
                Ok(Ok(out)) => self.success(out).await,
                Ok(Err(e))  => self.failure(e).await,
                Err(_)      => self.timeout().await,
            };

            delay_for(self.period).await;
        }
    }

    async fn query(&mut self) -> Result<Output> {
        let target = self.target.clone();
        let class  = DNSClass::IN;
        let record = self.record;

        let time = Instant::now();
        let res  = self.client.query(target, class, record).await?;
        let time = time.elapsed();

        match res.response_code() {
            ResponseCode::NoError => Ok(Output::new(time, res)),
            code                  => Err(anyhow!("{}", code)),
        }
    }

    async fn success(&self, out: Output) {
        debug!("{}: {}", self.task, out);
        self.envoy.export(record::Query {
            task: self.task,
            test: self.test,
            data: out.data,
            time: out.time,
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

#[derive(Debug)]
struct Output {
    data: String,
    time: Duration,
}

impl Output {
    fn new(time: Duration, res: DnsResponse) -> Self {
        let mut answers = res.answers().iter().map(|rec| {
            match rec.rdata() {
                RData::A(addr)     => addr.to_string(),
                RData::AAAA(addr)  => addr.to_string(),
                RData::ANAME(name) => name.to_string(),
                RData::CNAME(name) => name.to_string(),
                RData::MX(mx)      => mx.exchange().to_string(),
                RData::NS(name)    => name.to_string(),
                RData::PTR(name)   => name.to_string(),
                other              => format!("{:?}", other),
            }
        }).collect::<Vec<_>>();
        answers.sort_unstable();

        Self {
            data: answers.join(","),
            time: time,
        }
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} time {:?}", self.data, self.time)
    }
}