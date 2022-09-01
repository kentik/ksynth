use std::convert::TryInto;
use std::sync::Arc;
use anyhow::Result;
use http::uri::Uri;
use hyper::client::HttpConnector;
use hyper::service::Service;
use hyper_proxy::{Proxy, ProxyStream};
use rustls::ClientConfig;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};
use crate::net::tls::{Identity, Verifier};
use super::stream::Peer;

#[derive(Clone)]
pub struct ProxyConnector {
    connector: hyper_proxy::ProxyConnector<HttpConnector>,
}

impl ProxyConnector {
    pub fn new(config: Arc<ClientConfig>) -> Result<Self> {
        let mut http = HttpConnector::new();
        http.enforce_http(false);

        let mut c = hyper_proxy::ProxyConnector::unsecured(http);
        c.set_tls(Some(TlsConnector::from(config)));

        Ok(Self { connector: c })
    }

    pub fn add_proxy(&mut self, proxy: Proxy) {
        self.connector.add_proxy(proxy);
    }

    pub fn check(&self, uri: &Uri) -> Option<&Proxy> {
        self.connector.proxies().iter().find(|p| p.intercept().matches(uri))
    }

    pub async fn connect(&self, uri: &Uri) -> Result<ProxyStream<TcpStream>> {
        Ok(self.connector.clone().call(uri.clone()).await?)
    }
}

pub trait ProxyConnection {
    fn info(&self, host: &str, verifier: &Verifier) -> Result<(Peer, bool)>;
}

impl ProxyConnection for ProxyStream<TcpStream> {
    fn info(&self, host: &str, verifier: &Verifier) -> Result<(Peer, bool)> {
        match self {
            Self::NoProxy(s) => s.info(host, verifier),
            Self::Regular(s) => s.info(host, verifier),
            Self::Secured(s) => s.info(host, verifier),
        }
    }
}

impl ProxyConnection for TcpStream {
    fn info(&self, _host: &str, _verifier: &Verifier) -> Result<(Peer, bool)> {
        let addr   = self.peer_addr()?;
        let server = Identity::Unknown;
        let http2  = false;
        Ok((Peer { addr, server }, http2))
    }
}

impl ProxyConnection for TlsStream<TcpStream> {
    fn info(&self, host: &str, verifier: &Verifier) -> Result<(Peer, bool)> {
        let (tcp, tls) = self.get_ref();
        let addr   = tcp.peer_addr()?;
        let certs  = tls.peer_certificates().unwrap_or_default();
        let server = verifier.verify(certs, &host.try_into()?)?;
        let http2  = tls.alpn_protocol() == Some(b"h2");
        Ok((Peer { addr, server }, http2))
    }
}
