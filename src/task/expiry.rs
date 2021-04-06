use std::convert::TryFrom;
use std::time::Duration;

#[derive(Debug)]
pub struct Expiry {
    pub probe: Duration,
    pub task:  Duration,
}

impl Expiry {
    pub fn new(expiry: Duration, count: usize) -> Self {
        let count = u32::try_from(count).unwrap_or(1);
        Self {
            probe: expiry / count,
            task:  expiry,
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;
    use super::Expiry;

    #[test]
    fn expiries() {
        let expiry = Expiry::new(Duration::from_millis(2000), 1);
        assert_eq!(Duration::from_millis(2000), expiry.probe);
        assert_eq!(Duration::from_millis(2000), expiry.task);

        let expiry = Expiry::new(Duration::from_millis(2000), 2);
        assert_eq!(Duration::from_millis(1000), expiry.probe);
        assert_eq!(Duration::from_millis(2000), expiry.task);
    }
}
