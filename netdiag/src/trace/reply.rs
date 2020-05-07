use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use anyhow::Result;
use futures::{executor, ready};
use tokio::sync::oneshot::Receiver;
use tokio::time::Timeout;
use super::probe::Probe;
use super::state::State;

#[derive(Debug)]
pub enum Node {
    Node(u8, IpAddr, Duration),
    None(u8)
}

pub struct Reply {
    echo:  Timeout<Receiver<Echo>>,
    sent:  Instant,
    state: Arc<State>,
    probe: Probe,
}

#[derive(Debug)]
pub struct Echo(pub IpAddr, pub Instant, pub bool);

impl Reply {
    pub fn new(echo: Timeout<Receiver<Echo>>, sent: Instant, state: Arc<State>, probe: Probe) -> Reply {
        Self { echo, sent, probe, state }
    }

    async fn release(&self) {
        self.state.remove(&self.probe).await;
    }
}

impl Future for Reply {
    type Output = Result<Node>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let sent = self.sent;
        let n    = self.probe.ttl();
        let echo = Pin::new(&mut self.echo);

        let echo = match ready!(echo.poll(cx)) {
            Ok(Ok(echo)) => echo,
            Ok(Err(e))   => return Poll::Ready(Err(e.into())),
            Err(_)       => return Poll::Ready(Ok(Node::None(n))),
        };

        let addr = echo.0;
        let rtt  = echo.1.saturating_duration_since(sent);

        Poll::Ready(Ok(Node::Node(n, addr, rtt)))
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        executor::block_on(self.release());
    }
}
