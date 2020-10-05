use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use log::{debug, error};
use tokio::sync::mpsc::Receiver;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::system_conf::read_system_conf;
use synapi::agent::Net;
use synapi::tasks::{State, Config};
use synapi::tasks::{PingConfig, TraceConfig, FetchConfig, KnockConfig, QueryConfig};
use netdiag::{Bind, Knocker, Pinger, Tracer};
use crate::export::{Exporter, Target};
use crate::spawn::{Spawner, Handle};
use crate::status::Status;
use crate::task::{Network, Task, Resolver, Fetch, Fetcher, Knock, Ping, Query, Trace};
use crate::watch::{Event, Tasks};

pub struct Executor {
    tasks:    HashMap<u64, Handle>,
    rx:       Receiver<Event>,
    ex:       Arc<Exporter>,
    bind:     Bind,
    network:  Option<Network>,
    resolver: TokioAsyncResolver,
    status:   Arc<Status>,
    spawner:  Arc<Spawner>,
    pinger:   Arc<Pinger>,
    tracer:   Arc<Tracer>,
    fetcher:  Arc<Fetcher>,
    knocker:  Arc<Knocker>,
}

impl Executor {
    pub async fn new(rx: Receiver<Event>, ex: Arc<Exporter>, bind: Bind, net: Option<Network>) -> Result<Self> {
        let (config, options) = read_system_conf()?;

        let resolver = TokioAsyncResolver::tokio(config, options).await?;
        let status   = Arc::new(Status::default());
        let spawner  = Spawner::new(status.clone());

        let pinger   = Pinger::new(&bind).await?;
        let tracer   = Tracer::new(&bind).await?;
        let fetcher  = Fetcher::new(&bind)?;
        let knocker  = Knocker::new(&bind).await?;

        Ok(Self {
            tasks:    HashMap::new(),
            rx:       rx,
            ex:       ex,
            bind:     bind,
            network:  net,
            resolver: resolver,
            status:   status,
            spawner:  Arc::new(spawner),
            pinger:   Arc::new(pinger),
            tracer:   Arc::new(tracer),
            fetcher:  Arc::new(fetcher),
            knocker:  Arc::new(knocker),
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
        let resolver = Resolver::new(self.resolver.clone());

        for group in tasks {
            let target = Arc::new(Target {
                company: group.company,
                agent:   agent.id,
                device:  group.device,
                email:   group.kentik.email,
                token:   group.kentik.token,
            });

            for task in group.tasks {
                let id      = task.task;
                let test    = task.test;
                let config  = task.config;
                let state   = task.state;
                let family  = task.family;

                let network = self.network.unwrap_or_else(|| {
                    match family {
                        Net::IPv4 => Network::IPv4,
                        Net::IPv6 => Network::IPv6,
                        Net::Dual => Network::Dual,
                    }
                });

                let envoy  = self.ex.envoy(target.clone());
                let task   = Task::new(id, test, network, envoy, resolver.clone());

                let result = match state {
                    State::Created => self.insert(task, config).await,
                    State::Deleted => self.delete(id),
                    State::Updated => self.insert(task, config).await,
                };

                match result {
                    Ok(_)  => debug!("created task {}", id),
                    Err(e) => error!("invalid task {}: {}", id, e),
                }
            }
        }
        Ok(())
    }

    async fn insert(&mut self, task: Task, cfg: Config) -> Result<()> {
        let id = task.task;

        let handle = match cfg {
            Config::Ping(cfg)  => self.ping(id, task, cfg)?,
            Config::Trace(cfg) => self.trace(id, task, cfg)?,
            Config::Fetch(cfg) => self.fetch(id, task, cfg)?,
            Config::Knock(cfg) => self.knock(id, task, cfg)?,
            Config::Query(cfg) => self.query(id, task, cfg).await?,
            Config::Unknown    => Err(anyhow!("unsupported type"))?,
        };

        self.tasks.insert(id, handle);

        Ok(())
    }

    fn delete(&mut self, id: u64) -> Result<()> {
        debug!("deleted task {}", id);
        self.tasks.remove(&id);
        Ok(())
    }

    fn ping(&self, id: u64, task: Task, cfg: PingConfig) -> Result<Handle> {
        let ping = Ping::new(task, cfg, self.pinger.clone());
        Ok(self.spawner.spawn(id, ping.exec()))
    }

    fn trace(&self, id: u64, task: Task, cfg: TraceConfig) -> Result<Handle> {
        let trace = Trace::new(task, cfg, self.tracer.clone());
        Ok(self.spawner.spawn(id, trace.exec()))
    }

    fn fetch(&self, id: u64, task: Task, cfg: FetchConfig) -> Result<Handle> {
        let fetch = Fetch::new(task, cfg, self.fetcher.clone());
        Ok(self.spawner.spawn(id, fetch.exec()))
    }

    fn knock(&self, id: u64, task: Task, cfg: KnockConfig) -> Result<Handle> {
        let knock = Knock::new(task, cfg, self.knocker.clone());
        Ok(self.spawner.spawn(id, knock.exec()))
    }

    async fn query(&self, id: u64, task: Task, cfg: QueryConfig) -> Result<Handle> {
        let query = Query::new(task, cfg).await?;
        Ok(self.spawner.spawn(id, query.exec()))
    }
}
