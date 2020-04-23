use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use log::warn;
use tokio::sync::Mutex;
use tokio::time::interval;
use synapi::Client;
use super::{encode, Record, Target};

pub struct Exporter {
    export: Arc<Mutex<HashMap<Key, Output>>>,
    client: Client,
}

pub struct Envoy {
    export: Arc<Mutex<HashMap<Key, Output>>>,
    target: Arc<Target>,
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct Key {
    company: u64,
    device:  u64,
}

#[derive(Debug)]
struct Output {
    target: Arc<Target>,
    values: Vec<Record>,
}

impl Exporter {
    pub fn new(client: Client) -> Self {
        Self {
            export: Arc::new(Mutex::new(HashMap::new())),
            client: client,
        }
    }

    pub fn envoy(&self, target: Arc<Target>) -> Envoy {
        Envoy {
            export: self.export.clone(),
            target: target,
        }
    }

    pub async fn exec(self: Arc<Self>) -> Result<()> {
        let mut ticker = interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;

            for o in self.drain().await.values() {
                match self.send(&o.target, &o.values).await {
                    Ok(()) => (),
                    Err(e) => warn!("export failed: {:?}", e),
                }
            }
        }
    }

    async fn send(&self, target: &Target, records: &[Record]) -> Result<()> {
        let cid  = target.company;
        let did  = target.device.id;
        let name = "foo";

        let sid  = format!("{}:{}:{}", cid, name, did);
        let flow = encode(&target, &records)?;

        let email = &target.email;
        let token = &target.token;

        Ok(self.client.export(&sid, email, token, &flow).await?)
    }

    async fn drain(&self) -> HashMap<Key, Output> {
        let mut export = self.export.lock().await;
        let empty = HashMap::with_capacity(export.len());
        mem::replace(&mut export, empty)
    }
}

impl Envoy {
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
