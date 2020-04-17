use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use anyhow::Result;
use futures::{executor, ready};
use tokio::sync::oneshot::Receiver;
use super::ping::Token;
use super::state::State;

pub struct Pong {
    rtt:   Receiver<Instant>,
    sent:  Instant,
    state: State,
    token: Token,
}

impl Pong {
    pub fn new(rtt: Receiver<Instant>, sent: Instant, state: State, token: Token) -> Self {
        Self { rtt, sent, state, token }
    }

    async fn release(&self) {
        self.state.remove(&self.token).await;
    }
}

impl Future for Pong {
    type Output = Result<Duration>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.rtt).poll(cx)) {
            Ok(time) => Poll::Ready(Ok(time.duration_since(self.sent))),
            Err(e)   => Poll::Ready(Err(e.into())),
        }
    }
}

impl Drop for Pong {
    fn drop(&mut self) {
        executor::block_on(self.release());
    }
}
