use crate::export::Envoy;
use super::Resolver;

pub struct Task {
    pub task:     u64,
    pub test:     u64,
    pub envoy:    Envoy,
    pub resolver: Resolver,
}

impl Task {
    pub fn new(task: u64, test: u64, envoy: Envoy, resolver: Resolver) -> Self {
        Self { task, test, envoy, resolver }
    }
}
