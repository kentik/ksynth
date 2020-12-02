use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use rustls::ClientConfig;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};
use webpki::DNSNameRef;
use webpki_roots::TLS_SERVER_ROOTS;
use netdiag::Bind;

pub struct Shaker {
    bind:    Bind,
    connect: TlsConnector,
}

impl Shaker {
    pub fn new(bind: &Bind) -> Result<Self> {
        let mut cfg = ClientConfig::new();
        cfg.root_store.add_server_trust_anchors(&TLS_SERVER_ROOTS);

        Ok(Self {
            bind:    bind.clone(),
            connect: TlsConnector::from(Arc::new(cfg)),
        })
    }

    pub async fn shake(&self, name: DNSNameRef<'_>, addr: SocketAddr) -> Result<()> {
        self.connect(name, addr).await?;
        Ok(())
    }

    async fn connect(&self, name: DNSNameRef<'_>, addr: SocketAddr) -> Result<TlsStream<TcpStream>> {
        let (domain, bind) = match addr {
            SocketAddr::V4(_) => (Domain::ipv4(), self.bind.sa4().into()),
            SocketAddr::V6(_) => (Domain::ipv6(), self.bind.sa6().into()),
        };

        let socket = Socket::new(domain, Type::stream(), Some(Protocol::tcp()))?;
        socket.bind(&bind)?;

        let stream = socket.into_tcp_stream();
        let stream = TcpStream::connect_std(stream, &addr).await?;

        Ok(self.connect.connect(name, stream).await?)
    }
}
