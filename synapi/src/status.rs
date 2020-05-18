use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Report {
    tasks: Tasks,
}

#[derive(Debug, Serialize)]
pub struct Tasks {
    started: u64,
    running: u64,
    exited:  u64,
    failed:  u64,
}
