pub mod agent;
pub mod cmd;
pub mod version;

mod exec;
mod export;
mod spawn;
mod secure;
mod status;
mod stats;
mod task;
mod watch;

pub mod chf_capnp {
    include!(concat!(env!("OUT_DIR"), "/chf_capnp.rs"));
}
