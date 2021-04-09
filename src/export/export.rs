use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use log::info;
use tokio::sync::Mutex;
use synapi::Client;
use super::{Record, Target, influx, kentik};

#[derive(Clone)]
pub enum Exporter {
    Influx(Arc<influx::Exporter>),
    Kentik(Arc<kentik::Exporter>),
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
    pub fn influx(agent: String, endpoint: &str) -> Result<Self> {
        let export = influx::Exporter::new(agent, endpoint)?;
        Ok(Self::Influx(Arc::new(export)))
    }

    pub fn kentik(client: Arc<Client>) -> Result<Self> {
        let export = kentik::Exporter::new(client)?;
        Ok(Self::Kentik(Arc::new(export)))
    }

    pub fn envoy(&self, target: Arc<Target>) -> Envoy {
        match self {
            Self::Influx(export) => export.envoy(target),
            Self::Kentik(export) => export.envoy(target),
        }
    }

    pub async fn report(&self) {
        let queue = match self {
            Exporter::Influx(export) => export.queue().await,
            Exporter::Kentik(export) => export.queue().await,
        };

        let count = queue.len();
        let sum   = queue.values().map(|o| o.values.len()).sum::<usize>();

        info!("queue count {}, entries: {}", count, sum);
    }

    pub async fn exec(self) -> Result<()> {
        match self {
            Self::Influx(export) => export.exec().await,
            Self::Kentik(export) => export.exec().await,
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
