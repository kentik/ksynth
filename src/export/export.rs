use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::Mutex;
use synapi::Client;
use crate::cfg::Config;
use crate::output::Args;
use crate::status::Queue;
use super::{Record, Target, influx, kentik, newrelic};

#[derive(Clone)]
pub enum Exporter {
    Influx(Arc<influx::Exporter>),
    Kentik(Arc<kentik::Exporter>),
    NewRelic(Arc<newrelic::Exporter>),
}

pub struct Envoy {
    export: Arc<Mutex<HashMap<Key, Output>>>,
    target: Arc<Target>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Key {
    company: u64,
    device:  u64,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub target: Arc<Target>,
    pub values: Vec<Record>,
}

impl Exporter {
    pub fn influx(agent: String, cfg: &Config, args: Args) -> Result<Self> {
        let export = influx::Exporter::new(agent, cfg, args)?;
        Ok(Self::Influx(Arc::new(export)))
    }

    pub fn kentik(client: Arc<Client>) -> Result<Self> {
        let export = kentik::Exporter::new(client)?;
        Ok(Self::Kentik(Arc::new(export)))
    }

    pub fn newrelic(agent: String, cfg: &Config, args: Args) -> Result<Self> {
        let export = newrelic::Exporter::new(agent, cfg, args)?;
        Ok(Self::NewRelic(Arc::new(export)))
    }

    pub fn envoy(&self, target: Arc<Target>) -> Envoy {
        match self {
            Self::Influx(export)   => export.envoy(target),
            Self::Kentik(export)   => export.envoy(target),
            Self::NewRelic(export) => export.envoy(target),
        }
    }

    pub async fn report(&self) -> Queue {
        let queue = match self {
            Exporter::Influx(export)   => export.queue().await,
            Exporter::Kentik(export)   => export.queue().await,
            Exporter::NewRelic(export) => export.queue().await,
        };

        let length  = queue.len();
        let records = queue.values().map(|o| o.values.len()).sum();

        Queue { length, records }
    }

    pub async fn exec(self) -> Result<()> {
        match self {
            Self::Influx(export)   => export.exec().await,
            Self::Kentik(export)   => export.exec().await,
            Self::NewRelic(export) => export.exec().await,
        }
    }
}

impl Envoy {
    pub fn new(export: Arc<Mutex<HashMap<Key, Output>>>, target: Arc<Target>) -> Self {
        Self { export, target }
    }

    pub async fn export<T: Into<Record>>(&self, record: T) {
        let key = Key {
            company: self.target.company,
            device:  self.target.device.id,
        };

        let mut export = self.export.lock().await;

        export.entry(key).or_insert_with(|| {
            Output {
                target: self.target.clone(),
                values: Vec::new(),
            }
        }).values.push(record.into());
    }
}
