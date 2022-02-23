use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use rustls::{ClientConfig, RootCertStore, ServerName};
use tokio::net::{TcpSocket, TcpStream};
use tokio_rustls::{TlsConnector, client::TlsStream};
use netdiag::Bind;
use crate::task::Config;
use super::{Identity, Verifier};

pub struct Shaker {
    bind:     Bind,
    connect:  TlsConnector,
    verifier: Arc<Verifier>,
}

pub struct Connection {
    pub server: Identity,
    pub stream: TlsStream<TcpStream>,
}

impl Shaker {
    pub fn new(cfg: &Config) -> Result<Self> {
        let Config { bind, roots, .. } = cfg.clone();

        let verifier = Arc::new(Verifier::new(roots));

        let mut cfg = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        cfg.dangerous().set_certificate_verifier(verifier.clone());

        Ok(Self {
            bind:     bind,
            connect:  TlsConnector::from(Arc::new(cfg)),
            verifier: verifier,
        })
    }

    pub async fn shake(&self, name: &ServerName, addr: SocketAddr) -> Result<Connection> {
        self.connect(&name, addr).await
    }

    async fn connect(&self, name: &ServerName, addr: SocketAddr) -> Result<Connection> {
        let Self { bind, connect, verifier } = self;

        let (socket, bind) = match addr {
            SocketAddr::V4(_) => (TcpSocket::new_v4()?, bind.sa4()),
            SocketAddr::V6(_) => (TcpSocket::new_v6()?, bind.sa6()),
        };

        socket.bind(bind)?;

        let stream = socket.connect(addr).await?;
        let stream = connect.connect(name.clone(), stream).await?;

        let (_, tls) = stream.get_ref();
        let certs  = tls.peer_certificates().unwrap_or_default();
        let server = verifier.verify(&certs, &name)?;

        Ok(Connection { server, stream })
    }
}
