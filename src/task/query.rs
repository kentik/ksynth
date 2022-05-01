use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{Error, Result};
use tracing::{debug, info_span, warn, Instrument};
use netdiag::Bind;
use tokio::net::UdpSocket;
use tokio::time::{sleep, timeout};
use trust_dns_client::client::{AsyncClient, ClientHandle};
use trust_dns_client::op::{DnsResponse, ResponseCode};
use trust_dns_client::rr::{DNSClass, Name, RecordType, RData};
use trust_dns_client::udp::UdpClientStream;
use synapi::tasks::QueryConfig;
use crate::export::{record, Envoy};
use crate::status::Active;
use super::Task;

pub struct Query {
    task:   u64,
    test:   u64,
    target: Name,
    period: Duration,
    expiry: Duration,
    record: RecordType,
    envoy:  Envoy,
    client: AsyncClient,
    active: Arc<Active>,
}

impl Query {
    pub async fn new(task: Task, cfg: QueryConfig, bind: &Bind) -> Result<Self> {
        let expiry = cfg.expiry.into();
        let server = SocketAddr::from((&cfg.server.parse()?, cfg.port));

        let bind = Some(match server {
            SocketAddr::V4(_) => bind.sa4(),
            SocketAddr::V6(_) => bind.sa6(),
        });

        let stream = UdpClientStream::<UdpSocket>::with_bind_addr_and_timeout(server, bind, expiry);

        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        Ok(Self {
            task:   task.task,
            test:   task.test,
            target: cfg.target.parse()?,
            period: cfg.period.into(),
            expiry: expiry,
            record: cfg.record.parse()?,
            envoy:  task.envoy,
            client: client,
            active: task.active,
        })
    }

    pub async fn exec(mut self) -> Result<()> {
        loop {
            let task = self.task;
            let test = self.test;

            let span = info_span!("query", task, test);

            async {
                let expiry = self.expiry;
                let result = self.query(self.target.clone());

                match timeout(expiry, result).await {
                    Ok(Ok(out)) => self.success(out).await,
                    Ok(Err(e))  => self.failure(e).await,
                    Err(_)      => self.timeout().await,
                };
            }.instrument(span).await;

            sleep(self.period).await;
        }
    }

    async fn query(&mut self, target: Name) -> Result<Output> {
        let _guard = self.active.query();

        debug!("target {target}");

        let class  = DNSClass::IN;
        let record = self.record;

        let time = Instant::now();
        let res  = self.client.query(target, class, record).await?;
        let time = time.elapsed();

        Output::new(record, time, res)
    }

    async fn success(&self, out: Output) {
        debug!("{out}");
        self.envoy.export(record::Query {
            task:    self.task,
            test:    self.test,
            code:    out.code.into(),
            record:  out.record,
            answers: out.answers,
            time:    out.time,
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

#[derive(Debug)]
struct Output {
    code:    ResponseCode,
    record:  String,
    answers: String,
    time:    Duration,
}

impl Output {
    fn new(record: RecordType, time: Duration, res: DnsResponse) -> Result<Self> {
        let mut answers = res.answers().iter().map(|rec| {
            match rec.data() {
                Some(RData::A(addr))     => addr.to_string(),
                Some(RData::AAAA(addr))  => addr.to_string(),
                Some(RData::ANAME(name)) => name.to_string(),
                Some(RData::CNAME(name)) => name.to_string(),
                Some(RData::MX(mx))      => mx.exchange().to_string(),
                Some(RData::NS(name))    => name.to_string(),
                Some(RData::PTR(name))   => name.to_string(),
                Some(other)              => format!("{:?}", other),
                None                     => "none".to_string(),
            }
        }).collect::<Vec<_>>();
        answers.sort_unstable();

        let code    = res.response_code();
        let record  = record.to_string();
        let answers = serde_json::to_string(&answers)?;

        Ok(Self { code, record, answers, time })
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Self { code, record, answers, time } = self;
        write!(f, "{} {} time {:?}", record, match code {
            ResponseCode::NoError => answers as &dyn fmt::Display,
            _                     => code    as &dyn fmt::Display,
        }, time)
    }
}
