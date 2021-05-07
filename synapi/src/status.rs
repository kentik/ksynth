use std::collections::HashSet;
use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub system: System,
    pub tasks:  Tasks,
}

#[derive(Debug, Default, Serialize)]
pub struct System {
    pub load: f64,
    pub cpu:  f32,
    pub io:   f32,
    pub mem:  f32,
}

#[derive(Debug, Default, Serialize)]
pub struct Tasks {
    pub started: u64,
    pub running: u64,
    pub exited:  u64,
    pub failed:  u64,
    pub active:  HashSet<u64>,
}
