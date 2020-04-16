use std::future::Future;
use anyhow::Result;
use log::{debug, error};
use futures::future::{Abortable, AbortHandle};

pub fn spawn<F: Future<Output = Result<()>> + Send + 'static>(id: u64, future: F) -> Handle {
    let (abort, registration) = AbortHandle::new_pair();

    tokio::spawn(Abortable::new(async move {
        match future.await {
            Ok(()) => debug!("task {} finished", id),
            Err(e) => error!("task {} failed: {:?}", id, e),
        }
    }, registration));

    Handle { abort }
}

pub struct Handle {
    abort: AbortHandle,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.abort.abort();
    }
}
