use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use rustls::ClientConfig;
use tokio::net::{TcpSocket, TcpStream};
use tokio_rustls::{TlsConnector, client::TlsStream};
use webpki::DNSNameRef;
use netdiag::Bind;
use crate::task::Config;

pub struct Shaker {
    bind:    Bind,
    connect: TlsConnector,
}

impl Shaker {
    pub fn new(cfg: &Config) -> Result<Self> {
        let Config { bind, roots, .. } = cfg.clone();

        let mut cfg = ClientConfig::new();
        cfg.root_store = roots;

        Ok(Self {
            bind:    bind,
            connect: TlsConnector::from(Arc::new(cfg)),
        })
    }

    pub async fn shake(&self, name: DNSNameRef<'_>, addr: SocketAddr) -> Result<()> {
        self.connect(name, addr).await?;
        Ok(())
    }

    async fn connect(&self, name: DNSNameRef<'_>, addr: SocketAddr) -> Result<TlsStream<TcpStream>> {
        let (socket, bind) = match addr {
            SocketAddr::V4(_) => (TcpSocket::new_v4()?, self.bind.sa4()),
            SocketAddr::V6(_) => (TcpSocket::new_v6()?, self.bind.sa6()),
        };

        socket.bind(bind)?;

        let stream = socket.connect(addr).await?;
        Ok(self.connect.connect(name, stream).await?)
    }
}
