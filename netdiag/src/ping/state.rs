use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;
use tokio::sync::oneshot::Sender;
use super::ping::Token;

#[derive(Clone, Default)]
pub struct State(Arc<Mutex<HashMap<Token, Sender<Instant>>>>);

impl State {
    pub fn insert(&self, token: Token, tx: Sender<Instant>) {
        self.0.lock().insert(token, tx);
    }

    pub fn remove(&self, token: &Token) -> Option<Sender<Instant>> {
        self.0.lock().remove(token)
    }
}
