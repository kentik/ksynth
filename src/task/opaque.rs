use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Error, Result};
use serde_json::{Map, Value};
use tracing::{debug, warn, info_span, Instrument};
use tokio::time::{sleep, timeout};
use synapi::tasks::OpaqueConfig;
use crate::export::{record, Envoy};
use crate::script::{Export, Machine};
use crate::status::Active;
use super::Task;

pub struct Opaque {
    task:   u64,
    test:   u64,
    method: String,
    config: Value,
    period: Duration,
    expiry: Duration,
    envoy:  Envoy,
    active: Arc<Active>,
    export: Export,
}

impl Opaque {
    pub fn new(task: Task, cfg: OpaqueConfig, machine: Arc<Machine>) -> Result<Self> {
        let export = machine.find(&cfg.method)?;
        let period = cfg.period.into();
        let expiry = cfg.expiry.into();

        Ok(Self {
            task:   task.task,
            test:   task.test,
            method: cfg.method,
            config: cfg.config,
            period: period,
            expiry: expiry,
            envoy:  task.envoy,
            active: task.active,
            export: export,
        })
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            let task = self.task;
            let test = self.test;

            let span = info_span!("opaque", task, test);

            async {
                let _guard = self.active.opaque();

                let config = self.config.clone();
                let result = self.invoke(config);

                match timeout(self.expiry, result).await {
                    Ok(Ok(rtt)) => self.success(rtt).await,
                    Ok(Err(e))  => self.failure(e).await,
                    Err(_)      => self.timeout().await,
                };
            }.instrument(span).await;

            sleep(self.period).await;
        }
    }

    async fn invoke(&self, arg: Value) -> Result<Output> {
        let start = Instant::now();

        let data = match self.export.call(arg).await? {
            Value::Object(map) => Ok(map),
            value              => Err(anyhow!("invalid output: {value}")),
        };

        Ok(Output {
            data: data?,
            time: start.elapsed(),
        })
    }

    async fn success(&self, out: Output) {
        debug!("{} {:?}", self.method, out.time);
        self.envoy.export(record::Opaque {
            task:   self.task,
            test:   self.test,
            output: out.data,
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
    data: Map<String, Value>,
    time: Duration,
}
