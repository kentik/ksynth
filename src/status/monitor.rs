use std::sync::Arc;
use std::time::{Instant, Duration};
use anyhow::Result;
use log::{error, debug};
use parking_lot::Mutex;
use tokio::task::spawn;
use tokio::time::interval_at;
use synapi::{Client, Error};
use synapi::status::{Report, Tasks};
use crate::cfg::Config;
use super::{Addresses, Status, system};
use Error::Session;

pub struct Monitor {
    client: Arc<Client>,
    config: Config,
    status: Arc<Status>,
}

impl Monitor {
    pub fn new(client: Arc<Client>, status: Arc<Status>, config: Config) -> Result<Self> {
        Ok(Self { client, config, status })
    }

    pub async fn exec(self) -> Result<()> {
        let addresses = Addresses::new(self.config)?;
        let addrs     = Arc::new(Mutex::new(Vec::new()));

        spawn(addresses.exec(addrs.clone()));

        let delay = Duration::from_secs(30);
        let first = Instant::now() + delay;

        let mut ticker = interval_at(first.into(), delay);

        loop {
            ticker.tick().await;

            let snapshot = self.status.snapshot();
            let active   = snapshot.tasks.active.len();

            debug!("active tasks: {:?}", active);

            let report = Report {
                addrs:  addrs.lock().clone(),
                system: system().unwrap_or_default(),
                tasks:  Tasks {
                    started: snapshot.tasks.started,
                    running: snapshot.tasks.running,
                    exited:  snapshot.tasks.exited,
                    failed:  snapshot.tasks.failed,
                    active:  snapshot.tasks.active,
                },
            };

            match self.client.status(&report).await {
                Ok(_)        => debug!("status dispatched"),
                Err(Session) => debug!("not authenticated"),
                Err(e)       => error!("status delivery: {:?}", e),
            }
        }
    }
}
