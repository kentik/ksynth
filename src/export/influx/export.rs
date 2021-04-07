use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use log::warn;
use tokio::sync::Mutex;
use tokio::time::interval;
use crate::export::{Envoy, Key, Output, Record, Target};
use super::{Client, encode};

pub struct Exporter {
    export: Arc<Mutex<HashMap<Key, Output>>>,
    client: Arc<Client>,
    agent:  String,
}

impl Exporter {
    pub fn new(agent: String, endpoint: &str) -> Result<Self> {
        let client = Arc::new(Client::new(endpoint)?);
        let export = Arc::new(Mutex::new(HashMap::new()));
        Ok(Self { export, client, agent })
    }

    pub fn envoy(&self, target: Arc<Target>) -> Envoy {
        Envoy::new(self.export.clone(), target)
    }

    pub async fn exec(self: Arc<Self>) -> Result<()> {
        let mut ticker = interval(Duration::from_secs(10));
        let mut buffer = Vec::new();

        loop {
            ticker.tick().await;

            for o in self.drain().await.values() {
                match self.send(&o.values, &mut buffer).await {
                    Ok(()) => (),
                    Err(e) => warn!("export failed: {:?}", e),
                }
                buffer.clear();
            }
        }
    }

    async fn send(&self, records: &[Record], buffer: &mut Vec<u8>) -> Result<()> {
        encode(&self.agent, records, buffer)?;
        self.client.send(buffer).await?;
        Ok(())
    }

    async fn drain(&self) -> HashMap<Key, Output> {
        let mut export = self.export.lock().await;
        let empty = HashMap::with_capacity(export.len());
        mem::replace(&mut export, empty)
    }
}
