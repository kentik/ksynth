use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, oneshot::Sender};
use super::ping::Token;

#[derive(Clone, Default)]
pub struct State(Arc<Mutex<HashMap<Token, Sender<Instant>>>>);

impl State {
    pub async fn insert(&self, token: Token, tx: Sender<Instant>) {
        self.0.lock().await.insert(token, tx);
    }

    pub async fn remove(&self, token: &Token) -> Option<Sender<Instant>> {
        self.0.lock().await.remove(token)
    }
}
