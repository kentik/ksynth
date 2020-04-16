use std::net::Ipv4Addr;
use std::time::Duration;
use anyhow::Result;
use log::debug;
use tokio::time::delay_for;

pub struct Ping {
    id:   u64,
    addr: Ipv4Addr,
}

impl Ping {
    pub fn new(id: u64, addr: Ipv4Addr) -> Self {
        Self { id, addr }
    }

    pub async fn exec(self) -> Result<()> {
        let delay = Duration::from_secs(2);
        loop {
            debug!("{}: testing {}", self.id, self.addr);
            delay_for(delay).await;
        }
    }
}
