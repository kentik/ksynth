use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use log::warn;
use rustls::ClientConfig;
use tokio::sync::Mutex;
use tokio::time::interval;
use crate::cfg::Config;
use crate::export::{Envoy, Key, Output, Target};
use crate::output::Args;
use super::event::Client;

pub struct Exporter {
    export: Arc<Mutex<HashMap<Key, Output>>>,
    client: Arc<Client>,
}

impl Exporter {
    pub fn new(agent: String, cfg: &Config, args: Args) -> Result<Self> {
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(cfg.roots.clone())
            .with_no_client_auth();

        let client = Arc::new(Client::new(agent, config, args)?);
        let export = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self { export, client })
    }

    pub fn envoy(&self, target: Arc<Target>) -> Envoy {
        Envoy::new(self.export.clone(), target)
    }

    pub async fn queue(&self) -> HashMap<Key, Output> {
        self.export.lock().await.clone()
    }

    pub async fn exec(self: Arc<Self>) -> Result<()> {
        let mut ticker = interval(Duration::from_secs(10));

        loop {
            ticker.tick().await;

            for o in self.drain().await.values() {
                match self.client.send(&o.values).await {
                    Ok(()) => (),
                    Err(e) => warn!("export failed: {:?}", e),
                }
            }
        }
    }

    async fn drain(&self) -> HashMap<Key, Output> {
        let mut export = self.export.lock().await;
        let empty = HashMap::with_capacity(export.len());
        mem::replace(&mut export, empty)
    }
}
