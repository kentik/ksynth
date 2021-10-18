use std::net::SocketAddr;
use anyhow::Result;
use netdiag::Bind;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::LookupIpStrategy;
use trust_dns_resolver::system_conf::read_system_conf;
use crate::net::Resolver;
use crate::net::http::HttpClient;
use super::{server, Server};

pub async fn setup(host: &str, alpn: &[Vec<u8>]) -> Result<(HttpClient, Server)> {
    let (config, mut options) = read_system_conf()?;
    options.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;

    let resolver = Resolver::new(TokioAsyncResolver::tokio(config, options)?);
    let bind     = SocketAddr::new(host.parse()?, 0);

    let server   = server(bind, alpn).await?;
    let roots    = server.roots.clone();
    let client   = HttpClient::new(Bind::default(), resolver, roots)?;

    Ok((client, server))
}
