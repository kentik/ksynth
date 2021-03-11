use std::time::Duration;

#[derive(Debug)]
pub struct Expiry {
    pub probe: Duration,
    pub task:  Duration,
}

impl Expiry {
    pub fn new(expiry: u64, count: u64, limit: Option<u64>) -> Self {
        let limit = limit.unwrap_or(1);
        let probe = expiry / (count * limit);
        Self {
            probe: Duration::from_millis(probe),
            task:  Duration::from_millis(expiry),
        }
    }
}
