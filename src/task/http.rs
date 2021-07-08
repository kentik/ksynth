use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::io;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use anyhow::{anyhow, Error, Result};
use http::uri::{Port, Uri};
use hyper::{Body, Client, Request, Response};
use hyper::client::connect::{self, Connected};
use hyper::service::Service;
use rustls::{ClientConfig, Session};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpSocket, TcpStream};
use tokio::time::timeout;
use tokio_rustls::{TlsConnector, client::TlsStream};
use webpki::DNSNameRef;
use netdiag::Bind;
use super::{Config, Network, Resolver};
use super::tls::{Identity, Verifier};

#[derive(Clone)]
pub struct HttpClient {
    client: Client<Connector, Body>,
    expiry: Expiry,
}

#[derive(Clone)]
pub struct Expiry {
    pub connect: Duration,
    pub request: Duration,
}

impl HttpClient {
    pub fn new(cfg: &Config, expiry: Expiry) -> Result<Self> {
        let mut builder = Client::builder();
        builder.pool_idle_timeout(Duration::from_secs(30));
        builder.pool_max_idle_per_host(0);

        let conn   = Connector::new(cfg, expiry.connect);
        let client = builder.build(conn);

        Ok(Self { client, expiry })
    }

    pub async fn request(&self, req: Request<Body>) -> Result<Response<Body>> {
        let expiry   = self.expiry.request;
        let response = self.client.request(req);
        Ok(timeout(expiry, response).await??)
    }
}

#[derive(Clone)]
struct Connector {
    connect: Arc<Connect>,
    timeout: Duration,
}

struct Connect {
    bind:     Bind,
    network:  Network,
    resolver: Resolver,
    tls:      TlsConnector,
    verifier: Arc<Verifier>,
}

struct Connection {
    stream: Stream,
    times:  Times,
    server: Identity,
}

enum Stream {
    TCP(TcpStream),
    TLS(TlsStream<TcpStream>),
}

#[derive(Clone, Debug, Default)]
pub struct Times {
    pub dns: Duration,
    pub tcp: Duration,
    pub tls: Option<Duration>,
}

impl Connector {
    pub fn new(cfg: &Config, timeout: Duration) -> Self {
        let Config { bind, network, resolver, roots, .. } = cfg.clone();

        let verifier = Arc::new(Verifier::new(roots));

        let mut config = ClientConfig::new();
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        config.dangerous().set_certificate_verifier(verifier.clone());

        let network = network.unwrap_or(Network::Dual);
        let tls     = TlsConnector::from(Arc::new(config));
        let connect = Connect { bind, tls, network, resolver, verifier };

        Self {
            connect: Arc::new(connect),
            timeout: timeout,
        }
    }
}

impl Service<Uri> for Connector {
    type Response = Connection;
    type Error    = Error;
    type Future   = Pin<Box<dyn Future<Output = Result<Connection, Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let connect = self.connect.clone();
        let expiry  = self.timeout;
        Box::pin(async move {
            let connect = connect.connect(dst);
            timeout(expiry, connect).await?
        })
    }
}

impl Connect {
    async fn connect(self: Arc<Self>, uri: Uri) -> Result<Connection> {
        let Self { bind, network, resolver, tls, verifier } = &*self;

        let port = uri.port().as_ref().map(Port::as_u16);
        let port = match uri.scheme_str() {
            Some("http")  => port.unwrap_or(80),
            Some("https") => port.unwrap_or(443),
            _             => return Err(anyhow!("{}: invalid scheme", uri)),
        };

        let mut times = Times::default();

        let start = Instant::now();
        let addr  = match uri.host() {
            Some(host) => resolver.lookup(host, *network).await?,
            None       => return Err(anyhow!("{}: missing host", uri)),
        };

        times.dns = start.elapsed();

        let start  = Instant::now();
        let addr   = SocketAddr::new(addr, port);
        let socket = socket(&bind, &addr).await?;
        let stream = socket.connect(addr).await?;

        times.tcp = start.elapsed();

        if uri.scheme_str() != Some("https") {
            let stream = Stream::TCP(stream);
            let server = Identity::Unknown;
            return Ok(Connection { stream, times, server });
        }

        let hostname = uri.host().unwrap_or_default();
        let dnsname  = DNSNameRef::try_from_ascii_str(hostname)?;

        let start  = Instant::now();
        let stream = tls.connect(dnsname, stream).await?;

        times.tls = Some(start.elapsed());

        let (_, tls) = stream.get_ref();
        let certs  = tls.get_peer_certificates().unwrap_or_default();
        let server = verifier.verify(&certs, dnsname)?;
        let stream = Stream::TLS(stream);

        Ok(Connection { stream, times, server })
    }
}

async fn socket(bind: &Bind, addr: &SocketAddr) -> Result<TcpSocket> {
    let (socket, bind) = match addr {
        SocketAddr::V4(_) => (TcpSocket::new_v4()?, bind.sa4()),
        SocketAddr::V6(_) => (TcpSocket::new_v6()?, bind.sa6()),
    };

    socket.bind(bind)?;

    Ok(socket)
}

impl connect::Connection for Connection {
    fn connected(&self) -> Connected {
        match &self.stream {
            Stream::TCP(tcp) => tcp.connected(),
            Stream::TLS(tls) => {
                let (tcp, tls) = tls.get_ref();
                match tls.get_alpn_protocol() {
                    Some(b"h2") => tcp.connected().negotiated_h2(),
                    _           => tcp.connected(),
                }
            }.extra(self.server.clone())
        }.extra(self.times.clone())
    }
}

impl AsyncRead for Connection {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut ReadBuf<'_>) -> Poll<Result<(), io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_read(cx, buf),
            Stream::TLS(tls) => Pin::new(tls).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Connection {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_write(cx, buf),
            Stream::TLS(tls) => Pin::new(tls).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_flush(cx),
            Stream::TLS(tls) => Pin::new(tls).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_shutdown(cx),
            Stream::TLS(tls) => Pin::new(tls).poll_shutdown(cx),
        }
    }
}
