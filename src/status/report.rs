use std::sync::atomic::Ordering;
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Report {
    pub active: Active,
    pub export: Queue,
    pub tasks:  Vec<u64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Active {
    pub count: Count,
    pub tasks: Tasks,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Count {
    pub success: u64,
    pub failure: u64,
    pub timeout: u64,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Tasks {
    pub fetch: u64,
    pub knock: u64,
    pub ping:  u64,
    pub query: u64,
    pub shake: u64,
    pub trace: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Queue {
    pub length:  usize,
    pub records: usize,
}

impl Report {
    pub fn new(active: &super::Active, export: Queue, tasks: Vec<u64>) -> Self {
        let active = Active {
            count: Count {
                success: active.count.success.load(Ordering::Relaxed),
                failure: active.count.failure.load(Ordering::Relaxed),
                timeout: active.count.timeout.load(Ordering::Relaxed),
            },
            tasks: Tasks {
                fetch: active.tasks.fetch.load(Ordering::Relaxed),
                knock: active.tasks.knock.load(Ordering::Relaxed),
                ping:  active.tasks.ping.load(Ordering::Relaxed),
                query: active.tasks.query.load(Ordering::Relaxed),
                shake: active.tasks.shake.load(Ordering::Relaxed),
                trace: active.tasks.trace.load(Ordering::Relaxed),
            }
        };

        Self { active, export, tasks }
    }

    pub fn print(&self) {
        let counts = [
            self.active.count.success,
            self.active.count.failure,
            self.active.count.timeout,
        ].iter().map(u64::to_string).collect::<Vec<_>>();

        let active = [
            self.active.tasks.fetch,
            self.active.tasks.knock,
            self.active.tasks.ping,
            self.active.tasks.query,
            self.active.tasks.shake,
            self.active.tasks.trace,
        ];

        let pending = active.iter().sum::<u64>();
        let active  = active.iter().map(u64::to_string).collect::<Vec<_>>();

        info!("running {} tasks: {:?}", self.tasks.len(), self.tasks);
        info!("execution status: {}", counts.join(" / "));
        info!("pending {} count: {}", pending, active.join(" / "));

        let Queue { length, records } = self.export;

        info!("queue count {}, entries: {}", length, records);
    }
}
