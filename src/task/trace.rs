use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::Result;
use log::{debug, warn};
use tokio::time::delay_for;
use netdiag::{self, Node, Tracer};

pub struct Trace {
    id:     u64,
    addr:   Ipv4Addr,
    period: Duration,
    tracer: Arc<Tracer>,
}

impl Trace {
    pub fn new(id: u64, addr: Ipv4Addr, tracer: Arc<Tracer>) -> Self {
        let period = Duration::from_secs(10);
        Self { id, addr, period, tracer }
    }

    pub async fn exec(self) -> Result<()> {
        loop {
            debug!("{}: target {}", self.id, self.addr);

            let trace = netdiag::Trace {
                addr:   self.addr,
                probes: 3,
                limit:  32,
                expiry: Duration::from_millis(250),
            };
            let time = Instant::now();

            match self.tracer.route(trace).await {
                Ok(route) => self.report(route, time.elapsed()),
                Err(e)    => warn!("{}", e),
            }

            delay_for(self.period).await;
        }
    }

    fn report(&self, route: Vec<Vec<Node>>, time: Duration) {
        let hops = route.len();
        debug!("{}: {} hops in {:0.2?}", self.id, hops, time);
    }
}
