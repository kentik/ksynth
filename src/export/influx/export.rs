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
use crate::export::{Envoy, Key, Output, Record, Target};
use crate::output::Args;
use super::{client::{Auth, Client}, encode};

pub struct Exporter {
    export: Arc<Mutex<HashMap<Key, Output>>>,
    client: Arc<Client>,
    agent:  String,
}

impl Exporter {
    pub fn new(agent: String, cfg: &Config, args: Args) -> Result<Self> {
        let endpoint = args.get("endpoint")?;
        let token    = args.opt("token");
        let username = args.opt("username");
        let password = args.opt("password");
        let basic    = username.zip(password);

        let auth = if let Some(token) = token {
            Auth::Token(token.to_string())
        } else if let Some((username, password)) = basic {
            Auth::Basic(username.to_string(), password.to_string())
        } else {
            Auth::None
        };

        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(cfg.roots.clone())
            .with_no_client_auth();

        let client = Arc::new(Client::new(endpoint, config, auth)?);
        let export = Arc::new(Mutex::new(HashMap::new()));

        Ok(Self { export, client, agent })
    }

    pub fn envoy(&self, target: Arc<Target>) -> Envoy {
        Envoy::new(self.export.clone(), target)
    }

    pub async fn queue(&self) -> HashMap<Key, Output> {
        self.export.lock().await.clone()
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
