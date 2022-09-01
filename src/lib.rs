#![allow(clippy::module_inception, clippy::redundant_field_names)]

pub mod agent;
pub mod args;
pub mod cmd;
pub mod ctl;
pub mod net;
pub mod trace;
pub mod version;

mod cfg;
mod exec;
mod export;
mod output;
mod script;
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
