use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use log::{debug, error};
use tokio::sync::mpsc::Receiver;
use synapi::agent::Net;
use synapi::tasks::{State, Config};
use synapi::tasks::{PingConfig, TraceConfig, FetchConfig, KnockConfig};
use netdiag::{Bind, Knocker, Pinger, Tracer};
use crate::export::{Exporter, Envoy, Target};
use crate::spawn::{Spawner, Handle};
use crate::status::Status;
use crate::task::{Fetch, Fetcher, Knock, Ping, Trace};
use crate::watch::{Event, Tasks};

pub struct Executor {
    tasks:   HashMap<u64, Handle>,
    rx:      Receiver<Event>,
    ex:      Arc<Exporter>,
    bind:    Bind,
    network: Network,
    status:  Arc<Status>,
    spawner: Arc<Spawner>,
    pinger:  Arc<Pinger>,
    tracer:  Arc<Tracer>,
    fetcher: Arc<Fetcher>,
    knocker: Arc<Knocker>,
}

#[derive(Debug)]
pub struct Network {
    pub ip4: bool,
    pub ip6: bool,
    pub set: bool,
}

impl Executor {
    pub async fn new(rx: Receiver<Event>, ex: Arc<Exporter>, bind: Bind, net: Network) -> Result<Self> {
        let status  = Arc::new(Status::default());
        let spawner = Spawner::new(status.clone());

        let pinger  = Pinger::new(&bind).await?;
        let tracer  = Tracer::new(&bind).await?;
        let fetcher = Fetcher::new(&bind)?;
        let knocker = Knocker::new(&bind).await?;

        Ok(Self {
            tasks:   HashMap::new(),
            rx:      rx,
            ex:      ex,
            bind:    bind,
            network: net,
            status:  status,
            spawner: Arc::new(spawner),
            pinger:  Arc::new(pinger),
            tracer:  Arc::new(tracer),
            fetcher: Arc::new(fetcher),
            knocker: Arc::new(knocker),
        })
    }

    pub fn status(&self) -> Arc<Status> {
        self.status.clone()
    }

    pub async fn exec(mut self) -> Result<()> {
        debug!("IPv4 bind address {}", self.bind.sa4());
        debug!("IPv6 bind address {}", self.bind.sa6());

        while let Some(event) = self.rx.recv().await {
            match event {
                Event::Tasks(tasks) => self.tasks(tasks).await?,
                Event::Reset        => self.reset().await?
            }
        }

        Ok(())
    }

    async fn reset(&mut self) -> Result<()> {
        debug!("resetting task state");
        Ok(self.tasks.clear())
    }

    async fn tasks(&mut self, Tasks { agent, tasks }: Tasks) -> Result<()> {
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
        Ok(())
    }

    async fn insert(&mut self, id: u64, cfg: Config, envoy: Envoy) -> Result<()> {
        let handle = match cfg {
            Config::Ping(cfg)  => self.ping(id, cfg, envoy).await?,
            Config::Trace(cfg) => self.trace(id, cfg, envoy).await?,
            Config::Fetch(cfg) => self.fetch(id, cfg, envoy).await?,
            Config::Knock(cfg) => self.knock(id, cfg, envoy).await?,
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

    async fn knock(&self, id: u64, cfg: KnockConfig, envoy: Envoy) -> Result<Handle> {
        let Network { ip4, ip6, .. } = self.network;
        let client = self.knocker.clone();
        let knock = Knock::new(id, cfg, envoy, client);
        Ok(self.spawner.spawn(id, knock.exec(ip4, ip6)))
    }
}
