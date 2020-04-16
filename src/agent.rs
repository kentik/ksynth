use std::future::Future;
use anyhow::{Error, Result};
use ed25519_dalek::Keypair;
use tokio::sync::mpsc::{channel, Sender};
use synapi::Client;
use crate::exec::Executor;
use crate::watch::Watcher;

pub struct Agent {
    client: Client,
    keys:   Keypair,
}

impl Agent {
    pub fn new(client: Client, keys: Keypair) -> Self {
        Self { client, keys }
    }

    pub async fn exec(self) -> Option<Error> {
        let client = self.client;
        let keys   = self.keys;

        let (tx, mut rx) = channel(16);

        let (watcher, tasks) = Watcher::new(client, keys);
        let executor = Executor::new(tasks);

        spawn(watcher.exec(),  tx.clone());
        spawn(executor.exec(), tx.clone());

        rx.recv().await
    }
}

fn spawn<T: Future<Output = Result<()>> + Send + 'static>(task: T, mut tx: Sender<Error>) {
    tokio::spawn(async move {
        match task.await {
            Ok(()) => Ok(()),
            Err(e) => tx.send(e).await
        }
    });
}
