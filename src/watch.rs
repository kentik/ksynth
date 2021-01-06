use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use ed25519_compact::KeyPair;
use log::{debug, info, warn};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::sleep;
use synapi::{self, Client, Error, Retry};
use synapi::agent::Agent;
use synapi::auth::Auth;
use synapi::tasks::Group;
use synapi::Error::Unauthorized;

pub struct Watcher {
    client: Arc<Client>,
    keys:   KeyPair,
    output: Sender<Event>,
}

#[derive(Debug)]
pub enum Event {
    Tasks(Tasks),
    Reset,
}

#[derive(Debug)]
pub struct Tasks {
    pub agent: Agent,
    pub tasks: Vec<Group>,
}

impl Watcher {
    pub fn new(client: Arc<Client>, keys: KeyPair) -> (Self, Receiver<Event>) {
        let (tx, rx) = channel(128);
        (Self {
            client: client,
            keys:   keys,
            output: tx,
        }, rx)
    }

    pub async fn exec(mut self) -> Result<()> {
        let delay = Duration::from_secs(30);
        loop {
            if let Err(e) = self.watch().await {
                let err = e.downcast::<Error>()?;

                let delay = match err.retry() {
                    Retry::Delay(delay) => delay,
                    Retry::Default      => delay,
                    Retry::None         => return Err(err.into()),
                };

                warn!("{:?}", err);

                sleep(delay).await;
            }
        }
    }

    async fn watch(&mut self) -> Result<()> {
        let wait = Duration::from_secs(30);
        loop {
            match self.client.auth(&self.keys).await? {
                Auth::Ok(auth) => self.auth(auth.0).await?,
                Auth::Wait(c)  => self.wait(c, wait).await,
                Auth::Deny     => Err(Unauthorized)?,
            }
        }
    }

    async fn auth(&mut self, agent: Agent) -> Result<()> {
        debug!("authenticated agent {}", agent.id);
        self.tasks(agent).await
    }

    async fn wait(&mut self, challenge: String, delay: Duration) {
        info!("auth challenge: {}", challenge);
        debug!("waiting for authorization");
        sleep(delay).await;
    }

    async fn tasks(&mut self, agent: Agent) -> Result<()> {
        let delay = Duration::from_secs(60);
        let reset = Duration::from_secs(60 * 60 * 24);
        let start = Instant::now();

        let client = &mut self.client;
        let output = &mut self.output;

        let mut since = 0;
        while start.elapsed() < reset {
            debug!("requesting task updates");

            let tasks = client.tasks(since).await?;
            output.send(Event::Tasks(Tasks {
                agent: agent.clone(),
                tasks: tasks.groups,
            })).await?;

            since = tasks.timestamp;
            sleep(delay).await;
        }

        Ok(output.send(Event::Reset).await?)
    }
}
