use std::sync::Arc;
use std::time::{Instant, Duration};
use anyhow::Result;
use log::{error, debug};
use tokio::time::interval_at;
use synapi::{Client, Error};
use synapi::status::{Report, Tasks};
use super::Status;
use Error::Session;

pub struct Monitor {
    client: Arc<Client>,
    status: Arc<Status>,
}

impl Monitor {
    pub fn new(client: Arc<Client>, status: Arc<Status>) -> Self {
        Self { client, status }
    }

    pub async fn exec(self) -> Result<()> {
        let delay = Duration::from_secs(30);
        let first = Instant::now() + delay;

        let mut ticker = interval_at(first.into(), delay);

        loop {
            ticker.tick().await;

            let snapshot = self.status.snapshot();

            debug!("active tasks: {:?}", snapshot.tasks.active);

            let report = Report {
                tasks: Tasks {
                    started: snapshot.tasks.started,
                    running: snapshot.tasks.running,
                    exited:  snapshot.tasks.exited,
                    failed:  snapshot.tasks.failed,
                    active:  snapshot.tasks.active,
                },
            };

            match self.client.status(&report).await {
                Ok(_)        => debug!("status dispatched"),
                Err(Session) => debug!("not authenticated"),
                Err(e)       => error!("status delivery: {:?}", e),
            }
        }
    }
}
