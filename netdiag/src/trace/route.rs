use std::net::SocketAddrV4;
use std::time::Duration;
use anyhow::Result;
use futures::{Stream, StreamExt, TryStreamExt};
use futures::stream::unfold;
use super::probe::Probe;
use super::reply::Node;
use super::trace::Tracer;

pub struct Route<'t> {
    tracer: &'t Tracer,
    src:    SocketAddrV4,
    dst:    SocketAddrV4,
    expiry: Duration,
}

impl<'t> Route<'t> {
    pub fn new(tracer: &'t Tracer, src: SocketAddrV4, dst: SocketAddrV4, expiry: Duration) -> Route<'t> {
        Route { tracer, src, dst, expiry }
    }

    pub fn trace(&'t self, probes: usize) -> impl Stream<Item = Result<Vec<Node>>> + 't {
        unfold((self, self.dst, probes, 1), |(route, mut dst, probes, ttl)| async move {
            let stream = route.probe(&mut dst, ttl).take(probes);
            let result = stream.try_collect::<Vec<_>>().await;
            Some((result, (route, dst, probes, ttl + 1)))
        })
    }

    pub fn probe(&'t self, dst: &'t mut SocketAddrV4, ttl: u8) -> impl Stream<Item = Result<Node>> + 't {
        unfold((self, dst, ttl), |(route, dst, ttl)| async move {
            let Route { tracer, src, expiry, .. } = route;
            let probe  = Probe::new(*src, *dst, ttl);
            let result = tracer.probe(probe, *expiry).await;
            dst.set_port(dst.port() + 1);
            Some((result, (route, dst, ttl)))
        })
    }
}
