use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Report {
    pub tasks: Tasks,
}

#[derive(Debug, Serialize)]
pub struct Tasks {
    pub started: u64,
    pub running: u64,
    pub exited:  u64,
    pub failed:  u64,
}
