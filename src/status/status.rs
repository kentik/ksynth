use tokio::sync::Mutex;
use anyhow::{Error, Result};
use log::{debug, error};

#[derive(Debug, Default)]
pub struct Status {
    tasks: Mutex<Tasks>,
}

#[derive(Clone, Debug, Default)]
pub struct Tasks {
    pub started: u64,
    pub running: u64,
    pub exited:  u64,
    pub failed:  u64,
}

#[derive(Debug)]
pub struct Snapshot {
    pub tasks: Tasks,
}

impl Status  {
    pub async fn exec(&self, id: u64) {
        debug!("task {} started", id);
        let mut tasks = self.tasks.lock().await;
        tasks.started += 1;
        tasks.running += 1;
    }

    pub async fn exit(&self, id: u64, result: Result<()>) {
        let mut tasks = self.tasks.lock().await;
        match result {
            Ok(()) => self.success(id, &mut tasks),
            Err(e) => self.failure(id, &mut tasks, e),
        }
    }

    fn success(&self, id: u64, tasks: &mut Tasks) {
        debug!("task {} finished", id);
        tasks.running -= 1;
        tasks.exited  += 1;
    }

    fn failure(&self, id: u64, tasks: &mut Tasks, e: Error) {
        error!("task {} failed: {:?}", id, e);
        tasks.running -= 1;
        tasks.failed  += 1;
    }

    pub async fn snapshot(&self) -> Snapshot {
        let mut tasks = self.tasks.lock().await;

        let snapshot = Snapshot {
            tasks: tasks.clone(),
        };

        tasks.started = 0;
        tasks.exited  = 0;
        tasks.failed  = 0;

        snapshot
    }
}
