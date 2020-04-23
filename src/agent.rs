use std::future::Future;
use std::sync::Arc;
use anyhow::{Error, Result};
use ed25519_dalek::Keypair;
use tokio::sync::mpsc::{channel, Sender};
use synapi::Client;
use crate::exec::Executor;
use crate::export::Exporter;
use crate::watch::Watcher;

pub struct Agent {
    client: Client,
    keys:   Keypair,
}

impl Agent {
    pub fn new(client: Client, keys: Keypair) -> Self {
        Self { client, keys }
    }

    pub async fn exec(self) -> Result<()> {
        let client = self.client;
        let keys   = self.keys;

        let (tx, mut rx) = channel(16);

        let (watcher, tasks) = Watcher::new(client.clone(), keys);
        let exporter = Arc::new(Exporter::new(client));
        let executor = Executor::new(tasks, exporter.clone()).await?;

        spawn(watcher.exec(),  tx.clone());
        spawn(exporter.exec(), tx.clone());
        spawn(executor.exec(), tx.clone());

        match rx.recv().await {
            Some(e) => Err(e),
            None    => Ok(()),
        }
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
