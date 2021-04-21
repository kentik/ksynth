use std::sync::atomic::{AtomicU64, Ordering};

pub struct Active {
    pub count: Count,
    pub tasks: Tasks,
}

#[derive(Default)]
pub struct Count {
    pub success: AtomicU64,
    pub failure: AtomicU64,
    pub timeout: AtomicU64,
}

#[derive(Default)]
pub struct Tasks {
    pub fetch: AtomicU64,
    pub knock: AtomicU64,
    pub ping:  AtomicU64,
    pub query: AtomicU64,
    pub shake: AtomicU64,
    pub trace: AtomicU64,
}

pub struct Guard<'a>(&'a AtomicU64);

impl Active {
    pub fn new() -> Self {
        Self {
            count: Default::default(),
            tasks: Default::default(),
        }
    }

    pub fn fetch(&self) -> Guard<'_> {
        Guard::new(&self.tasks.fetch)
    }

    pub fn knock(&self) -> Guard<'_> {
        Guard::new(&self.tasks.knock)
    }

    pub fn ping(&self) -> Guard<'_> {
        Guard::new(&self.tasks.ping)
    }

    pub fn query(&self) -> Guard<'_> {
        Guard::new(&self.tasks.query)
    }

    pub fn shake(&self) -> Guard<'_> {
        Guard::new(&self.tasks.shake)
    }

    pub fn trace(&self) -> Guard<'_> {
        Guard::new(&self.tasks.trace)
    }

    pub fn success(&self) {
        self.count.success.fetch_add(1, Ordering::Relaxed);
    }

    pub fn failure(&self) {
        self.count.failure.fetch_add(1, Ordering::Relaxed);
    }

    pub fn timeout(&self) {
        self.count.timeout.fetch_add(1, Ordering::Relaxed);
    }
}

impl<'a> Guard<'a> {
    fn new(count: &'a AtomicU64) -> Self {
        count.fetch_add(1, Ordering::Relaxed);
        Self(count)
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
