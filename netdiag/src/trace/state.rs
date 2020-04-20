use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::ops::Deref;
use futures::executor;
use rand::prelude::*;
use rand::distributions::Uniform;
use tokio::sync::Mutex;
use tokio::sync::oneshot::Sender;
use super::probe::Probe;
use super::reply::Echo;

const PORT_MIN: u16 = 33434;
const PORT_MAX: u16 = 65407;

#[derive(Debug)]
pub struct State {
    range:  Uniform<u16>,
    source: Mutex<HashMap<SocketAddrV4, ()>>,
    state:  Mutex<HashMap<Key, Sender<Echo>>>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct Key(SocketAddrV4, SocketAddrV4);

#[derive(Debug)]
pub struct Lease<'s>(&'s State, SocketAddrV4);

impl State {
    pub fn new() -> Self {
        Self {
            range:  Uniform::new(PORT_MIN, PORT_MAX),
            source: Default::default(),
            state:  Default::default(),
        }
    }

    pub async fn reserve(&self, src: Ipv4Addr, dst: Ipv4Addr) -> (Lease<'_>, SocketAddrV4) {
        loop {
            let port = thread_rng().sample(self.range);
            let src  = SocketAddrV4::new(src, port);
            let dst  = SocketAddrV4::new(dst, PORT_MIN);

            let mut set = self.source.lock().await;

            if let Entry::Vacant(e) = set.entry(src) {
                let src = Lease(self, src);
                e.insert(());
                return (src, dst);
            }
        }
    }

    pub async fn release(&self, src: &SocketAddrV4) {
        self.source.lock().await.remove(src);
    }

    pub async fn insert(&self, probe: &Probe, tx: Sender<Echo>) {
        let key = Key(probe.src, probe.dst);
        self.state.lock().await.insert(key, tx);
    }

    pub async fn remove(&self, probe: &Probe) -> Option<Sender<Echo>> {
        let key = Key(probe.src, probe.dst);
        self.state.lock().await.remove(&key)
    }
}

impl Deref for Lease<'_> {
    type Target = SocketAddrV4;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        executor::block_on(self.0.release(&self.1));
    }
}
