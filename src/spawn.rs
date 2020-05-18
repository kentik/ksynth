use std::future::Future;
use std::sync::Arc;
use anyhow::Result;
use futures::future::{Abortable, Aborted, AbortHandle};
use crate::status::Status;

pub struct Spawner {
    status: Arc<Status>
}

pub struct Handle {
    handle: AbortHandle
}

impl Spawner {
    pub fn new(status: Arc<Status>) -> Self {
        Self { status }
    }

    pub fn spawn<F: Future<Output = Result<()>> + Send + 'static>(&self, id: u64, task: F) -> Handle {
        let (handle, registration) = AbortHandle::new_pair();
        let status = self.status.clone();

        let task = Abortable::new(task, registration);

        tokio::spawn(async move {
            status.exec(id).await;
            let r = match task.await {
                Ok(result)   => result,
                Err(Aborted) => Ok(())
            };
            status.exit(id, r).await;
        });

        Handle { handle }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
