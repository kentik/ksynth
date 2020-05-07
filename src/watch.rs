use std::time::Duration;
use anyhow::{Error, Result};
use ed25519_dalek::Keypair;
use log::{debug, warn};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::delay_for;
use synapi::{self, Client};
use synapi::auth::Auth;
use synapi::tasks::Group;
use synapi::Error::{Application, Unauthorized};

pub struct Watcher {
    client: Client,
    keys:   Keypair,
    output: Sender<Vec<Group>>,
}

impl Watcher {
    pub fn new(client: Client, keys: Keypair) -> (Self, Receiver<Vec<Group>>) {
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
            let result = self.watch().await;

            match retry(result)? {
                Some(e) => warn!("{:?}", e),
                None    => continue,
            };

            delay_for(delay).await;
        }
    }

    async fn watch(&mut self) -> Result<()> {
        let wait = Duration::from_secs(30);
        let ver  = env!("CARGO_PKG_VERSION");
        loop {
            match self.client.auth(&self.keys, ver).await? {
                Auth::Ok(s) => self.tasks(&s).await?,
                Auth::Wait  => self.wait(wait).await,
                Auth::Deny  => Err(Unauthorized)?,
            }
        }
    }

    async fn wait(&mut self, delay: Duration) {
        debug!("waiting for authorization");
        delay_for(delay).await;
    }

    async fn tasks(&mut self, session: &str) -> Result<()> {
        let delay  = Duration::from_secs(60);

        let client = &mut self.client;
        let output = &mut self.output;

        let mut since = 0;
        loop {
            debug!("requesting task updates");

            let tasks = client.tasks(session, since).await?;
            output.send(tasks.groups).await?;

            since = tasks.timestamp;
            delay_for(delay).await;
        }
    }
}

fn retry(result: Result<()>) -> Result<Option<Error>> {
   match result.map_err(Error::downcast::<synapi::Error>)  {
       Ok(())                       => Ok(None),
       Err(Ok(e @ Application(..))) => Err(e)?,
       Err(Ok(e @ Unauthorized))    => Err(e)?,
       Err(Ok(e))                   => Ok(Some(e.into())),
       Err(Err(e))                  => Err(e)?,
    }
}
