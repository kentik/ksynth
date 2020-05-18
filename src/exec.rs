use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use log::{debug, error};
use tokio::sync::mpsc::Receiver;
use synapi::agent::Net;
use synapi::tasks::{State, Config};
use synapi::tasks::{PingConfig, TraceConfig, FetchConfig};
use netdiag::{Pinger, Tracer};
use crate::export::{Exporter, Envoy, Target};
use crate::spawn::{Spawner, Handle};
use crate::status::Status;
use crate::task::{Fetcher, Ping, Trace, Fetch};
use crate::watch::Update;

pub struct Executor {
    tasks:   HashMap<u64, Handle>,
    rx:      Receiver<Update>,
    ex:      Arc<Exporter>,
    network: Network,
    status:  Arc<Status>,
    spawner: Arc<Spawner>,
    pinger:  Arc<Pinger>,
    tracer:  Arc<Tracer>,
    fetcher: Arc<Fetcher>,
}

#[derive(Debug)]
struct Network {
    ip4: bool,
    ip6: bool,
    set: bool,
}

impl Executor {
    pub async fn new(rx: Receiver<Update>, ex: Arc<Exporter>, ip4: bool, ip6: bool) -> Result<Self> {
        let network = Network {
            ip4: ip4,
            ip6: ip6,
            set: !ip4 || !ip6,
        };

        let status  = Arc::new(Status::default());
        let spawner = Spawner::new(status.clone());

        Ok(Self {
            tasks:   HashMap::new(),
            rx:      rx,
            ex:      ex,
            network: network,
            status:  status,
            spawner: Arc::new(spawner),
            pinger:  Arc::new(Pinger::new()?),
            tracer:  Arc::new(Tracer::new().await?),
            fetcher: Arc::new(Fetcher::new()?),
        })
    }

    pub fn status(&self) -> Arc<Status> {
        self.status.clone()
    }

    pub async fn exec(mut self) -> Result<()> {
        while let Some(Update { agent, tasks }) = self.rx.recv().await {
            if !self.network.set {
                let (ip4, ip6) = match agent.net {
                    Net::IPv4 => (true,  false),
                    Net::IPv6 => (false, true ),
                    Net::Dual => (true,  true ),
                };

                self.network.ip4 = ip4;
                self.network.ip6 = ip6;
            }

            for group in tasks {
                let target = Arc::new(Target {
                    company: group.company,
                    agent:   agent.id,
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
                        Ok(_)  => debug!("created task {}", task.id),
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
        let Network { ip4, ip6, .. } = self.network;
        let pinger = self.pinger.clone();
        let ping = Ping::new(id, cfg, envoy, pinger);
        Ok(self.spawner.spawn(id, ping.exec(ip4, ip6)))
    }

    async fn trace(&self, id: u64, cfg: TraceConfig, envoy: Envoy) -> Result<Handle> {
        let Network { ip4, ip6, .. } = self.network;
        let tracer = self.tracer.clone();
        let trace = Trace::new(id, cfg, envoy, tracer);
        Ok(self.spawner.spawn(id, trace.exec(ip4, ip6)))
    }

    async fn fetch(&self, id: u64, cfg: FetchConfig, envoy: Envoy) -> Result<Handle> {
        let client = self.fetcher.clone();
        let fetch = Fetch::new(id, cfg, envoy, client);
        Ok(self.spawner.spawn(id, fetch.exec()))
    }
}
