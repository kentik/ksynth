use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use anyhow::{anyhow, Result};
use log::{debug, error};
use tokio::net::lookup_host;
use tokio::sync::mpsc::Receiver;
use synapi::tasks::{Group, State, Config};
use synapi::tasks::{PingConfig, TraceConfig, FetchConfig};
use crate::task::{spawn, Handle, Ping, Trace, Fetch};

pub struct Executor {
    tasks: HashMap<u64, Handle>,
    rx:    Receiver<Vec<Group>>,
}

impl Executor {
    pub fn new(rx: Receiver<Vec<Group>>) -> Self {
        Self {
            tasks: HashMap::new(),
            rx:    rx,
        }
    }

    pub async fn exec(mut self) -> Result<()> {
        while let Some(update) = self.rx.recv().await {
            for group in update {
                for task in group.tasks {
                    let result = match task.state {
                        State::Created => self.insert(task.id, task.config).await,
                        State::Deleted => self.delete(task.id),
                        State::Updated => self.insert(task.id, task.config).await,
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

    async fn insert(&mut self, id: u64, cfg: Config) -> Result<()> {
        let handle = match cfg {
            Config::Ping(cfg)  => self.ping(id, cfg).await?,
            Config::Trace(cfg) => self.trace(id, cfg).await?,
            Config::Fetch(cfg) => self.fetch(id, cfg).await?,
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


    async fn ping(&self, id: u64, cfg: PingConfig) -> Result<Handle> {
        let addr = resolve(&cfg.target).await?;
        let ping = Ping::new(id, addr);
        Ok(spawn(id, ping.exec()))
    }

    async fn trace(&self, id: u64, cfg: TraceConfig) -> Result<Handle> {
        let addr  = resolve(&cfg.target).await?;
        let trace = Trace::new(id, addr);
        Ok(spawn(id, trace.exec()))
    }

    async fn fetch(&self, id: u64, cfg: FetchConfig) -> Result<Handle> {
        let addr  = resolve(&cfg.target).await?;
        let fetch = Fetch::new(id, addr);
        Ok(spawn(id, fetch.exec()))
    }

}

async fn resolve(host: &str) -> Result<Ipv4Addr> {
    let addr = format!("{}:0", host);
    for addr in lookup_host(&addr).await? {
        if let SocketAddr::V4(addr) = addr {
            return Ok(*addr.ip());
        }
    }
    Err(anyhow!("no IPv4 addr for {}", host))
}
