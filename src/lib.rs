#![allow(clippy::module_inception, clippy::redundant_field_names)]

pub mod agent;
pub mod args;
pub mod cmd;
pub mod net;
pub mod version;

mod exec;
mod export;
mod output;
mod spawn;
mod secure;
mod status;
mod stats;
mod task;
mod update;
mod watch;

pub mod chf_capnp {
    include!(concat!(env!("OUT_DIR"), "/chf_capnp.rs"));
}
