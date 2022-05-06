use std::collections::HashMap;
use std::sync::{Arc, atomic::Ordering};
use anyhow::{anyhow, Result};
use log::{debug, error, info};
use tokio::sync::mpsc::Receiver;
use synapi::agent::Net;
use synapi::tasks::{State, TaskConfig};
use synapi::tasks::{FetchConfig, KnockConfig, PingConfig, QueryConfig, ShakeConfig, TraceConfig};
use netdiag::{Bind, Knocker, Pinger, Tracer};
use crate::cfg::Config;
use crate::export::{Exporter, Target};
use crate::net::{Network, Resolver};
use crate::net::tls::Shaker;
use crate::spawn::{Spawner, Handle};
use crate::status::{Active, Status};
use crate::task::{Task, Fetcher};
use crate::task::{Fetch, Knock, Ping, Query, Shake, Trace};
use crate::watch::{Event, Tasks};

pub struct Executor {
    tasks:    HashMap<u64, Handle>,
    rx:       Receiver<Event>,
    ex:       Exporter,
    bind:     Bind,
    network:  Option<Network>,
    resolver: Resolver,
    active:   Arc<Active>,
    status:   Arc<Status>,
    spawner:  Arc<Spawner>,
    fetcher:  Arc<Fetcher>,
    knocker:  Arc<Knocker>,
    pinger:   Arc<Pinger>,
    shaker:   Arc<Shaker>,
    tracer:   Arc<Tracer>,
}

impl Executor {
    pub async fn new(rx: Receiver<Event>, ex: Exporter, cfg: Config) -> Result<Self> {
        let Config { bind, network, resolver, .. } = cfg.clone();

        let active  = Arc::new(Active::new());
        let status  = Arc::new(Status::default());
        let spawner = Spawner::new(status.clone());

        let fetcher = Fetcher::new(&cfg)?;
        let knocker = Knocker::new(&bind).await?;
        let pinger  = Pinger::new(&bind).await?;
        let shaker  = Shaker::new(&cfg)?;
        let tracer  = Tracer::new(&bind).await?;

        Ok(Self {
            tasks:    HashMap::new(),
            rx:       rx,
            ex:       ex,
            bind:     bind,
            network:  network,
            active:   active,
            resolver: resolver,
            status:   status,
            spawner:  Arc::new(spawner),
            fetcher:  Arc::new(fetcher),
            knocker:  Arc::new(knocker),
            pinger:   Arc::new(pinger),
            shaker:   Arc::new(shaker),
            tracer:   Arc::new(tracer),
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
                Event::Reset        => self.reset().await?,
                Event::Report       => self.report().await,
            }
        }

        Ok(())
    }

    async fn reset(&mut self) -> Result<()> {
        debug!("resetting task state");
        self.tasks.clear();
        Ok(())
    }

    async fn tasks(&mut self, Tasks { agent, tasks }: Tasks) -> Result<()> {
        let resolver = self.resolver.clone();

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

                let active   = self.active.clone();
                let envoy    = self.ex.envoy(target.clone());
                let resolver = resolver.clone();

                let task = Task::new(active, id, test, network, envoy, resolver);

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

    async fn insert(&mut self, task: Task, cfg: TaskConfig) -> Result<()> {
        let id = task.task;

        let handle = match cfg {
            TaskConfig::Fetch(cfg) => self.fetch(id, task, cfg)?,
            TaskConfig::Knock(cfg) => self.knock(id, task, cfg)?,
            TaskConfig::Ping(cfg)  => self.ping(id, task, cfg)?,
            TaskConfig::Query(cfg) => self.query(id, task, cfg).await?,
            TaskConfig::Shake(cfg) => self.shake(id, task, cfg)?,
            TaskConfig::Trace(cfg) => self.trace(id, task, cfg)?,
            _                      => Err(anyhow!("unsupported type"))?,
        };

        self.tasks.insert(id, handle);

        Ok(())
    }

    fn delete(&mut self, id: u64) -> Result<()> {
        debug!("deleted task {}", id);
        self.tasks.remove(&id);
        Ok(())
    }

    fn fetch(&self, id: u64, task: Task, cfg: FetchConfig) -> Result<Handle> {
        let fetch = Fetch::new(task, cfg, self.fetcher.clone())?;
        Ok(self.spawner.spawn(id, fetch.exec()))
    }

    fn knock(&self, id: u64, task: Task, cfg: KnockConfig) -> Result<Handle> {
        let knock = Knock::new(task, cfg, self.knocker.clone());
        Ok(self.spawner.spawn(id, knock.exec()))
    }

    fn ping(&self, id: u64, task: Task, cfg: PingConfig) -> Result<Handle> {
        let ping = Ping::new(task, cfg, self.pinger.clone());
        Ok(self.spawner.spawn(id, ping.exec()))
    }

    async fn query(&self, id: u64, task: Task, cfg: QueryConfig) -> Result<Handle> {
        let query = Query::new(task, cfg, &self.bind).await?;
        Ok(self.spawner.spawn(id, query.exec()))
    }

    #[allow(dead_code)]
    fn shake(&self, id: u64, task: Task, cfg: ShakeConfig) -> Result<Handle> {
        let shake = Shake::new(task, cfg, self.shaker.clone());
        Ok(self.spawner.spawn(id, shake.exec()))
    }

    fn trace(&self, id: u64, task: Task, cfg: TraceConfig) -> Result<Handle> {
        let trace = Trace::new(task, cfg, self.tracer.clone());
        Ok(self.spawner.spawn(id, trace.exec()))
    }

    async fn report(&mut self) {
        let mut tasks = self.tasks.keys().collect::<Vec<_>>();
        tasks.sort_unstable();

        let counts = [
            self.active.count.success.swap(0, Ordering::Relaxed),
            self.active.count.failure.swap(0, Ordering::Relaxed),
            self.active.count.timeout.swap(0, Ordering::Relaxed),
        ].iter().map(u64::to_string).collect::<Vec<_>>();

        let active = [
            self.active.tasks.fetch.load(Ordering::Relaxed),
            self.active.tasks.knock.load(Ordering::Relaxed),
            self.active.tasks.ping.load(Ordering::Relaxed),
            self.active.tasks.query.load(Ordering::Relaxed),
            self.active.tasks.shake.load(Ordering::Relaxed),
            self.active.tasks.trace.load(Ordering::Relaxed),
        ];

        let pending = active.iter().sum::<u64>();
        let active  = active.iter().map(u64::to_string).collect::<Vec<_>>();

        info!("running {} tasks: {:?}", tasks.len(), tasks);
        info!("execution status: {}", counts.join(" / "));
        info!("pending {} count: {}", pending, active.join(" / "));

        self.ex.report().await;
    }
}
