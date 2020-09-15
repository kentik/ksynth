use crate::export::Envoy;

pub struct Task {
    pub task:  u64,
    pub test:  u64,
    pub envoy: Envoy,
}

impl Task {
    pub fn new(task: u64, test: u64, envoy: Envoy) -> Self {
        Self { task, test, envoy }
    }
}
