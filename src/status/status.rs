use std::collections::HashSet;
use anyhow::{Error, Result};
use log::{debug, error};
use parking_lot::Mutex;

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
    pub active:  HashSet<u64>,
}

#[derive(Debug)]
pub struct Snapshot {
    pub tasks: Tasks,
}

impl Status  {
    pub fn exec(&self, id: u64) {
        debug!("task {} started", id);
        let mut tasks = self.tasks.lock();
        tasks.started += 1;
        tasks.running += 1;
        tasks.active.insert(id);
    }

    pub fn exit(&self, id: u64, result: Result<()>) {
        let mut tasks = self.tasks.lock();
        match result {
            Ok(()) => self.success(id, &mut tasks),
            Err(e) => self.failure(id, &mut tasks, e),
        }
        tasks.active.remove(&id);
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

    pub fn snapshot(&self) -> Snapshot {
        let mut tasks = self.tasks.lock();

        let snapshot = Snapshot {
            tasks: tasks.clone(),
        };

        tasks.started = 0;
        tasks.exited  = 0;
        tasks.failed  = 0;

        snapshot
    }
}
