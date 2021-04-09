use std::sync::atomic::{AtomicU64, Ordering};

pub struct Active {
    pub fetch: AtomicU64,
    pub knock: AtomicU64,
    pub ping:  AtomicU64,
    pub query: AtomicU64,
    pub shake: AtomicU64,
    pub trace: AtomicU64,
}

pub struct Guard<'a>(&'a AtomicU64);

pub struct Report {
    pub fetch: u64,
    pub knock: u64,
    pub ping:  u64,
    pub query: u64,
    pub shake: u64,
    pub trace: u64,
}

impl Active {
    pub fn new() -> Self {
        Self {
            fetch: AtomicU64::new(0),
            knock: AtomicU64::new(0),
            ping:  AtomicU64::new(0),
            query: AtomicU64::new(0),
            shake: AtomicU64::new(0),
            trace: AtomicU64::new(0),
        }
    }

    pub fn fetch(&self) -> Guard<'_> {
        Guard::new(&self.fetch)
    }

    pub fn knock(&self) -> Guard<'_> {
        Guard::new(&self.knock)
    }

    pub fn ping(&self) -> Guard<'_> {
        Guard::new(&self.ping)
    }

    pub fn query(&self) -> Guard<'_> {
        Guard::new(&self.query)
    }

    pub fn shake(&self) -> Guard<'_> {
        Guard::new(&self.shake)
    }

    pub fn trace(&self) -> Guard<'_> {
        Guard::new(&self.trace)
    }

    pub fn report(&self) -> Report {
        Report {
            fetch: self.fetch.load(Ordering::Relaxed),
            knock: self.knock.load(Ordering::Relaxed),
            ping:  self.ping.load(Ordering::Relaxed),
            query: self.query.load(Ordering::Relaxed),
            shake: self.shake.load(Ordering::Relaxed),
            trace: self.trace.load(Ordering::Relaxed),
        }
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
