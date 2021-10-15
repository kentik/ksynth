use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::str::FromStr;
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
use crate::net::{Network, Resolver};
use crate::net::tls::{Identity, Verifier};
use super::Config;

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
        let Config { bind, resolver, roots, .. } = cfg.clone();

        let verifier = Arc::new(Verifier::new(roots));

        let mut config = ClientConfig::new();
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        config.dangerous().set_certificate_verifier(verifier.clone());

        let tls     = TlsConnector::from(Arc::new(config));
        let connect = Connect { bind, tls, resolver, verifier };

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

#[derive(Debug, PartialEq)]
struct Target {
    scheme:  Scheme,
    host:    String,
    port:    u16,
    network: Network,
}

#[derive(Debug, PartialEq)]
enum Scheme {
    HTTP,
    HTTPS,
}

impl Connect {
    async fn connect(self: Arc<Self>, uri: Uri) -> Result<Connection> {
        let Self { bind, resolver, tls, verifier } = &*self;

        let Target { scheme, host, port, network } = Target::new(uri)?;

        let mut times = Times::default();

        let start = Instant::now();
        let addr  = resolver.lookup(&host, network).await?;

        times.dns = start.elapsed();

        let start  = Instant::now();
        let addr   = SocketAddr::new(addr, port);
        let socket = socket(bind, &addr).await?;
        let stream = socket.connect(addr).await?;

        times.tcp = start.elapsed();

        if scheme == Scheme::HTTP {
            let stream = Stream::TCP(stream);
            let server = Identity::Unknown;
            return Ok(Connection { stream, times, server });
        }

        let dnsname = DNSNameRef::try_from_ascii_str(&host)?;

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

impl Target {
    fn new(uri: Uri) -> Result<Self> {
        let (network, scheme) = match uri.scheme_str().and_then(|s| s.split_once('+')) {
            Some(("ipv4", scheme)) => (Network::IPv4, scheme.parse()?),
            Some(("ipv6", scheme)) => (Network::IPv6, scheme.parse()?),
            Some(("dual", scheme)) => (Network::Dual, scheme.parse()?),
            _                      => return Err(anyhow!("{}: invalid scheme", uri)),
        };

        let host = match uri.host() {
            Some(host) => host.to_owned(),
            None       => return Err(anyhow!("{}: missing host", uri)),
        };

        let port = uri.port().as_ref().map(Port::as_u16);
        let port = match scheme {
            Scheme::HTTP  => port.unwrap_or(80),
            Scheme::HTTPS => port.unwrap_or(443),
        };

        Ok(Self { scheme, host, port, network })
    }
}

impl FromStr for Scheme {
    type Err = Error;

    fn from_str(scheme: &str) -> Result<Self, Self::Err> {
        match scheme {
            "http"  => Ok(Self::HTTP),
            "https" => Ok(Self::HTTPS),
            _       => Err(anyhow!("{}: unsupported scheme", scheme))
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use crate::net::Network;
    use super::{Scheme, Target};

    #[test]
    fn target_scheme() -> Result<()> {
        assert_eq!(Target {
            scheme: Scheme::HTTP,
            host:   "foo.com".into(),
            port:   80,
            network: Network::IPv4,
        }, Target::new("ipv4+http://foo.com".parse()?)?);

        assert_eq!(Target {
            scheme: Scheme::HTTPS,
            host:   "foo.com".into(),
            port:   443,
            network: Network::IPv4,
        }, Target::new("ipv4+https://foo.com".parse()?)?);

        assert!(Target::new("//foo.com".parse()?).is_err());
        assert!(Target::new("ssh://foo.com".parse()?).is_err());

        Ok(())
    }

    #[test]
    fn target_port() -> Result<()> {
        assert_eq!(Target {
            scheme: Scheme::HTTP,
            host:   "foo.com".into(),
            port:   80,
            network: Network::IPv4,
        }, Target::new("ipv4+http://foo.com".parse()?)?);

        assert_eq!(Target {
            scheme: Scheme::HTTP,
            host:   "foo.com".into(),
            port:   8888,
            network: Network::IPv4,
        }, Target::new("ipv4+http://foo.com:8888".parse()?)?);

        assert_eq!(Target {
            scheme: Scheme::HTTPS,
            host:   "foo.com".into(),
            port:   443,
            network: Network::IPv4,
        }, Target::new("ipv4+https://foo.com".parse()?)?);

        assert_eq!(Target {
            scheme: Scheme::HTTPS,
            host:   "foo.com".into(),
            port:   4433,
            network: Network::IPv4,
        }, Target::new("ipv4+https://foo.com:4433".parse()?)?);

        Ok(())
    }

    #[test]
    fn target_network() -> Result<()> {
        assert_eq!(Target {
            scheme: Scheme::HTTP,
            host:   "foo.com".into(),
            port:   80,
            network: Network::IPv4,
        }, Target::new("ipv4+http://foo.com".parse()?)?);

        assert_eq!(Target {
            scheme: Scheme::HTTP,
            host:   "foo.com".into(),
            port:   80,
            network: Network::IPv6,
        }, Target::new("ipv6+http://foo.com".parse()?)?);

        assert_eq!(Target {
            scheme: Scheme::HTTP,
            host:   "foo.com".into(),
            port:   80,
            network: Network::Dual,
        }, Target::new("dual+http://foo.com".parse()?)?);

        assert!(Target::new("http://foo.com".parse()?).is_err());
        assert!(Target::new("ipv5+http://foo.com".parse()?).is_err());

        Ok(())
    }
}
