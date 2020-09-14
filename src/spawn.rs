use std::future::Future;
use std::sync::Arc;
use anyhow::Result;
use futures::future::{Abortable, AbortHandle};
use crate::status::Status;

pub struct Spawner {
    status: Arc<Status>
}

pub struct Handle {
    id:     u64,
    handle: AbortHandle,
    status: Arc<Status>,
}

impl Spawner {
    pub fn new(status: Arc<Status>) -> Self {
        Self { status }
    }

    pub fn spawn<F: Future<Output = Result<()>> + Send + 'static>(&self, id: u64, task: F) -> Handle {
        let (handle, registration) = AbortHandle::new_pair();
        let status = self.status.clone();

        tokio::spawn(Abortable::new(async move {
            status.exec(id);
            let r = task.await;
            status.exit(id, r);
        }, registration));

        Handle { id, handle, status: self.status.clone() }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.status.stop(self.id);
        self.handle.abort();
    }
}
