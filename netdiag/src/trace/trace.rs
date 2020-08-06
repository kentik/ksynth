use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use futures::future;
use futures::{StreamExt, TryStreamExt};
use tokio::sync::oneshot::channel;
use tokio::time::timeout;
use crate::Bind;
use super::{probe::Probe, reply::{Node, Reply}, route::Route};
use super::{sock4::Sock4, sock6::Sock6};
use super::state::State;

#[derive(Debug)]
pub struct Trace {
    pub addr:   IpAddr,
    pub probes: usize,
    pub limit:  usize,
    pub expiry: Duration,
}

pub struct Tracer {
    sock4: Sock4,
    sock6: Sock6,
    state: Arc<State>,
}

impl Tracer {
    pub async fn new(bind: &Bind) -> Result<Self> {
        let state = Arc::new(State::new());

        let sock4 = Sock4::new(bind, state.clone()).await?;
        let sock6 = Sock6::new(bind, state.clone()).await?;

        Ok(Self {
            sock4: sock4,
            sock6: sock6,
            state: state,
        })
    }

    pub async fn route(&self, trace: Trace) -> Result<Vec<Vec<Node>>> {
        let Trace { addr, probes, limit, expiry } = trace;

        let src = self.source(addr).await?;
        let (src, dst) = self.state.reserve(src, addr).await;
        let route = Route::new(self, *src, dst, expiry);

        let mut done = false;
        Ok(route.trace(probes).take_while(|result| {
            let last = done;
            if let Ok(nodes) = result {
                done = nodes.iter().any(|node| {
                    match node {
                        Node::Node(_, ip, _) => ip == &addr,
                        Node::None(_)        => false,
                    }
                });
            }
            future::ready(!last)
        }).take(limit).try_collect().await?)
    }

    pub async fn probe(&self, probe: Probe, expiry: Duration) -> Result<Node> {
        let state = self.state.clone();

        let (tx, rx) = channel();
        state.insert(&probe, tx);

        let sent = self.send(&probe).await?;
        let echo = timeout(expiry, rx);
        Reply::new(echo, sent, state, probe).await
    }

    pub async fn send(&self, probe: &Probe) -> Result<Instant> {
        match probe {
            Probe::V4(v4) => self.sock4.send(v4).await,
            Probe::V6(v6) => self.sock6.send(v6).await,
        }
    }

    pub async fn source(&self, dst: IpAddr) -> Result<IpAddr> {
        match dst {
            IpAddr::V4(..) => self.sock4.source(dst).await,
            IpAddr::V6(..) => self.sock6.source(dst).await,
        }
    }
}
