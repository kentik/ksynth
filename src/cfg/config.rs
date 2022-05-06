use std::convert::{TryFrom, TryInto};
use std::fs::{metadata, File};
use std::mem::take;
use std::time::{Duration, SystemTime};
use anyhow::{Error, Result};
use netdiag::Bind;
use rustls::RootCertStore;
use tokio::sync::mpsc::Sender;
use tokio::time::interval;
use tracing::{debug, error};
use synapi::agent::Agent;
use synapi::tasks::{Device, Group, Kentik, State, Task};
use crate::net::{Network, Resolver};
use crate::watch::{self, Event};
use super::schema::{self, Tasks};

#[derive(Clone)]
pub struct Config {
    pub bind:     Bind,
    pub network:  Option<Network>,
    pub resolver: Resolver,
    pub roots:    RootCertStore,
    pub tasks:    Option<String>,
}

impl Config {
    pub fn watcher(&self) -> Option<Watcher> {
        let agent   = 0;
        let device  = 0;

        let email   = String::new();
        let token   = String::new();
        let network = self.network.unwrap_or_default().into();
        let columns = Vec::new();

        let agent   = Agent { id: agent, net: network };
        let device  = Device { id: device, columns };
        let kentik  = Kentik { email, token };

        let config  = self.tasks.as_ref()?.clone();
        let updated = SystemTime::UNIX_EPOCH;
        let tasks   = Vec::new();

        Some(Watcher { config, agent, device, kentik, tasks, updated })
    }
}

pub struct Watcher {
    config:  String,
    agent:   Agent,
    device:  Device,
    kentik:  Kentik,
    tasks:   Vec<Task>,
    updated: SystemTime,
}

impl Watcher {
    pub async fn exec(mut self, events: Sender<Event>) -> Result<()> {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            let modified = metadata(&self.config)?.modified()?;
            let delay    = Duration::from_secs(1);

            if modified > self.updated && modified.elapsed()? > delay {
                match self.reload(&events, modified).await {
                    Ok(()) => debug!("load complete"),
                    Err(e) => error!("load failed: {e}"),
                };
            }

            interval.tick().await;
        }
    }

    async fn reload(&mut self, events: &Sender<Event>, timestamp: SystemTime) -> Result<()> {
        debug!("loading tasks from {}", self.config);

        let file  = File::open(&self.config)?;
        let tasks = serde_yaml::from_reader::<_, Tasks>(&file)?;

        let tasks = tasks.tasks.into_iter().enumerate().map(|(index, task)| {
            Ok(Task {
                task:   index.try_into()?,
                test:   0,
                config: task.config.try_into()?,
                family: task.network.try_into()?,
                state:  State::Created,
            })
        }).collect::<Result<Vec<_>>>()?;

        let mut outdated = take(&mut self.tasks);
        for task in &mut outdated {
            task.state = State::Deleted;
        };

        events.send(Event::Tasks(watch::Tasks {
            agent: self.agent.clone(),
            tasks: vec![Group {
                company: 0,
                kentik:  self.kentik.clone(),
                device:  self.device.clone(),
                tasks:   outdated,
            }],
        })).await?;

        events.send(Event::Tasks(watch::Tasks {
            agent: self.agent.clone(),
            tasks: vec![Group {
                company: 0,
                kentik:  self.kentik.clone(),
                device:  self.device.clone(),
                tasks:   tasks.clone(),
            }],
        })).await?;

        self.tasks   = tasks;
        self.updated = timestamp;

        Ok(())
    }
}

impl TryFrom<schema::Config> for synapi::tasks::TaskConfig {
    type Error = Error;

    fn try_from(c: schema::Config) -> Result<Self, Self::Error> {
        Ok(match c {
            schema::Config::Fetch(c) => Self::Fetch(c.try_into()?),
            schema::Config::Knock(c) => Self::Knock(c.try_into()?),
            schema::Config::Ping (c) => Self::Ping (c.try_into()?),
            schema::Config::Query(c) => Self::Query(c.try_into()?),
            schema::Config::Shake(c) => Self::Shake(c.try_into()?),
            schema::Config::Trace(c) => Self::Trace(c.try_into()?),
        })
    }
}

impl TryFrom<schema::Fetch> for synapi::tasks::FetchConfig {
    type Error = Error;

    fn try_from(c: schema::Fetch) -> Result<Self, Self::Error> {
        Ok(Self {
            target:   c.target,
            method:   c.method,
            body:     c.body,
            headers:  c.headers,
            insecure: c.insecure,
            period:   c.period.try_into()?,
            expiry:   c.expiry.try_into()?,
        })
    }
}

impl TryFrom<schema::Knock> for synapi::tasks::KnockConfig {
    type Error = Error;

    fn try_from(c: schema::Knock) -> Result<Self, Self::Error> {
        Ok(Self {
            target: c.target,
            period: c.period.try_into()?,
            count:  c.count.try_into()?,
            expiry: c.expiry.try_into()?,
            delay:  c.delay.try_into()?,
            port:   c.port,
        })
    }
}

impl TryFrom<schema::Ping> for synapi::tasks::PingConfig {
    type Error = Error;

    fn try_from(c: schema::Ping) -> Result<Self, Self::Error> {
        Ok(Self {
            target: c.target,
            period: c.period.try_into()?,
            count:  c.count.try_into()?,
            delay:  c.delay.try_into()?,
            expiry: c.expiry.try_into()?,
        })
    }
}

impl TryFrom<schema::Query> for synapi::tasks::QueryConfig {
    type Error = Error;

    fn try_from(c: schema::Query) -> Result<Self, Self::Error> {
        Ok(Self {
            target: c.target,
            period: c.period.try_into()?,
            expiry: c.expiry.try_into()?,
            server: c.server,
            port:   c.port,
            record: c.record,
        })
    }
}

impl TryFrom<schema::Shake> for synapi::tasks::ShakeConfig {
    type Error = Error;

    fn try_from(c: schema::Shake) -> Result<Self, Self::Error> {
        Ok(Self {
            target: c.target,
            port:   c.port,
            period: c.period.try_into()?,
            expiry: c.expiry.try_into()?,
        })
    }
}

impl TryFrom<schema::Trace> for synapi::tasks::TraceConfig {
    type Error = Error;

    fn try_from(c: schema::Trace) -> Result<Self, Self::Error> {
        Ok(Self {
            protocol: c.protocol,
            port:     c.port,
            target:   c.target,
            period:   c.period.try_into()?,
            count:    c.count.try_into()?,
            limit:    c.limit.try_into()?,
            delay:    c.delay.try_into()?,
            expiry:   c.expiry.try_into()?,
        })
    }
}

impl TryFrom<schema::Count> for synapi::tasks::Count {
    type Error = Error;

    fn try_from(c: schema::Count) -> Result<Self, Self::Error> {
        Ok(usize::try_from(c.0)?.into())
    }
}

impl TryFrom<schema::Count> for synapi::tasks::Limit {
    type Error = Error;

    fn try_from(c: schema::Count) -> Result<Self, Self::Error> {
        Ok(usize::try_from(c.0)?.into())
    }
}

impl TryFrom<schema::Time> for synapi::tasks::Delay {
    type Error = Error;

    fn try_from(c: schema::Time) -> Result<Self, Self::Error> {
        Ok(c.0.into())
    }
}

impl TryFrom<schema::Time> for synapi::tasks::Expiry {
    type Error = Error;

    fn try_from(c: schema::Time) -> Result<Self, Self::Error> {
        Ok(c.0.into())
    }
}

impl TryFrom<schema::Time> for synapi::tasks::Period {
    type Error = Error;

    fn try_from(c: schema::Time) -> Result<Self, Self::Error> {
        Ok(c.0.into())
    }
}

impl From<Network> for synapi::agent::Net {
    fn from(net: Network) -> Self {
        match net {
            Network::IPv4 => Self::IPv4,
            Network::IPv6 => Self::IPv6,
            Network::Dual => Self::Dual,
        }
    }
}
