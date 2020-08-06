use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::{IpAddr, SocketAddr};
use std::ops::Deref;
use rand::prelude::*;
use rand::distributions::Uniform;
use parking_lot::Mutex;
use tokio::sync::oneshot::Sender;
use tokio::task;
use super::probe::Probe;
use super::reply::Echo;

const PORT_MIN: u16 = 33434;
const PORT_MAX: u16 = 65407;

#[derive(Debug)]
pub struct State {
    range:  Uniform<u16>,
    source: Mutex<HashMap<SocketAddr, ()>>,
    state:  Mutex<HashMap<Key, Sender<Echo>>>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct Key(SocketAddr, SocketAddr);

#[derive(Debug)]
pub struct Lease<'s>(&'s State, SocketAddr);

impl State {
    pub fn new() -> Self {
        Self {
            range:  Uniform::new(PORT_MIN, PORT_MAX),
            source: Default::default(),
            state:  Default::default(),
        }
    }

    pub async fn reserve(&self, src: IpAddr, dst: IpAddr) -> (Lease<'_>, SocketAddr) {
        loop {
            let port = thread_rng().sample(self.range);
            let src  = SocketAddr::new(src, port);
            let dst  = SocketAddr::new(dst, PORT_MIN);

            if let Entry::Vacant(e) = self.source.lock().entry(src) {
                let src = Lease(self, src);
                e.insert(());
                return (src, dst);
            }

            task::yield_now().await;
        }
    }

    pub fn release(&self, src: &SocketAddr) {
        self.source.lock().remove(src);
    }

    pub fn insert(&self, probe: &Probe, tx: Sender<Echo>) {
        let key = Key(probe.src(), probe.dst());
        self.state.lock().insert(key, tx);
    }

    pub fn remove(&self, probe: &Probe) -> Option<Sender<Echo>> {
        let key = Key(probe.src(), probe.dst());
        self.state.lock().remove(&key)
    }
}

impl Deref for Lease<'_> {
    type Target = SocketAddr;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.0.release(&self.1);
    }
}
