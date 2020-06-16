use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use ed25519_dalek::Keypair;
use log::{debug, warn};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::delay_for;
use synapi::{self, Client, Error, Retry};
use synapi::agent::Agent;
use synapi::auth::Auth;
use synapi::tasks::Group;
use synapi::Error::Unauthorized;

pub struct Watcher {
    client: Arc<Client>,
    name:   String,
    keys:   Keypair,
    output: Sender<Update>,
}

#[derive(Debug)]
pub struct Update {
    pub agent: Agent,
    pub tasks: Vec<Group>,
}

impl Watcher {
    pub fn new(client: Arc<Client>, name: String, keys: Keypair) -> (Self, Receiver<Update>) {
        let (tx, rx) = channel(128);
        (Self {
            client: client,
            name:   name,
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

                delay_for(delay).await;
            }
        }
    }

    async fn watch(&mut self) -> Result<()> {
        let wait = Duration::from_secs(30);
        loop {
            match self.client.auth(&self.name, &self.keys).await? {
                Auth::Ok(auth) => self.auth(auth.0).await?,
                Auth::Wait     => self.wait(wait).await,
                Auth::Deny     => Err(Unauthorized)?,
            }
        }
    }

    async fn auth(&mut self, agent: Agent) -> Result<()> {
        debug!("authenticated agent {}", agent.id);
        self.tasks(agent).await
    }

    async fn wait(&mut self, delay: Duration) {
        debug!("waiting for authorization");
        delay_for(delay).await;
    }

    async fn tasks(&mut self, agent: Agent) -> Result<()> {
        let delay  = Duration::from_secs(60);

        let client = &mut self.client;
        let output = &mut self.output;

        let mut since = 0;
        loop {
            debug!("requesting task updates");

            let tasks = client.tasks(since).await?;
            output.send(Update {
                agent: agent.clone(),
                tasks: tasks.groups,
            }).await?;

            since = tasks.timestamp;
            delay_for(delay).await;
        }
    }
}
