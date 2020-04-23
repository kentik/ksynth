use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use log::{debug, error};
use tokio::sync::mpsc::Receiver;
use synapi::tasks::{Group, State, Config};
use synapi::tasks::{PingConfig, TraceConfig, FetchConfig};
use netdiag::{Pinger, Tracer};
use crate::export::{Exporter, Envoy, Target};
use crate::task::{spawn, Handle, Ping, Trace, Fetch};
use crate::task::Fetcher;

pub struct Executor {
    tasks:   HashMap<u64, Handle>,
    rx:      Receiver<Vec<Group>>,
    ex:      Arc<Exporter>,
    pinger:  Arc<Pinger>,
    tracer:  Arc<Tracer>,
    fetcher: Arc<Fetcher>,
}

impl Executor {
    pub async fn new(rx: Receiver<Vec<Group>>, ex: Arc<Exporter>) -> Result<Self> {
        Ok(Self {
            tasks:   HashMap::new(),
            rx:      rx,
            ex:      ex,
            pinger:  Arc::new(Pinger::new()?),
            tracer:  Arc::new(Tracer::new().await?),
            fetcher: Arc::new(Fetcher::new()?),
        })
    }

    pub async fn exec(mut self) -> Result<()> {
        while let Some(update) = self.rx.recv().await {
            for group in update {
                let target = Arc::new(Target {
                    company: group.company,
                    device:  group.device,
                    email:   group.kentik.email,
                    token:   group.kentik.token,
                });

                for task in group.tasks {
                    let envoy = self.ex.envoy(target.clone());

                    let result = match task.state {
                        State::Created => self.insert(task.id, task.config, envoy).await,
                        State::Deleted => self.delete(task.id),
                        State::Updated => self.insert(task.id, task.config, envoy).await,
                    };

                    match result {
                        Ok(_)  => debug!("started task {}", task.id),
                        Err(e) => error!("invalid task {}: {}", task.id, e),
                    }
                }
            }
        }
        Ok(())
    }

    async fn insert(&mut self, id: u64, cfg: Config, envoy: Envoy) -> Result<()> {
        let handle = match cfg {
            Config::Ping(cfg)  => self.ping(id, cfg, envoy).await?,
            Config::Trace(cfg) => self.trace(id, cfg, envoy).await?,
            Config::Fetch(cfg) => self.fetch(id, cfg, envoy).await?,
            _                  => Err(anyhow!("unsupported type"))?,
        };

        self.tasks.insert(id, handle);

        Ok(())
    }

    fn delete(&mut self, id: u64) -> Result<()> {
        debug!("deleted task {}", id);
        self.tasks.remove(&id);
        Ok(())
    }


    async fn ping(&self, id: u64, cfg: PingConfig, envoy: Envoy) -> Result<Handle> {
        let target = cfg.target;
        let pinger = self.pinger.clone();

        let ping = Ping::new(id, target, envoy, pinger);
        Ok(spawn(id, ping.exec()))
    }

    async fn trace(&self, id: u64, cfg: TraceConfig, envoy: Envoy) -> Result<Handle> {
        let target = cfg.target;
        let tracer = self.tracer.clone();

        let trace = Trace::new(id, target, envoy, tracer);
        Ok(spawn(id, trace.exec()))
    }

    async fn fetch(&self, id: u64, cfg: FetchConfig, envoy: Envoy) -> Result<Handle> {
        let target = cfg.target;
        let client = self.fetcher.clone();

        let fetch = Fetch::new(id, target, envoy, client);
        Ok(spawn(id, fetch.exec()))
    }

}
