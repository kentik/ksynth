use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use anyhow::{anyhow, Error, Result};
use http::uri::{Port, Scheme, Uri};
use hyper::{Body, Client, Request, Response};
use hyper::service::Service;
use hyper_rustls::HttpsConnector;
use rustls::ClientConfig;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::TcpStream;
use tokio::time::timeout;
use netdiag::Bind;
use super::{Config, Network, Resolver};

#[derive(Clone)]
pub struct HttpClient {
    client: Client<HttpsConnector<Connector>, Body>,
    expiry: Expiry,
}

#[derive(Clone)]
pub struct Expiry {
    pub connect: Duration,
    pub request: Duration,
}

impl HttpClient {
    pub fn new(cfg: &Config, expiry: Expiry) -> Result<Self> {
        let Config { bind, network, resolver, roots } = cfg.clone();

        let mut cfg = ClientConfig::new();
        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        cfg.root_store     = roots;

        let mut builder = Client::builder();
        builder.pool_idle_timeout(Duration::from_secs(30));
        builder.pool_max_idle_per_host(1);

        let net    = network.unwrap_or(Network::Dual);
        let http   = Connector::new(bind, net, resolver, expiry.connect);
        let https  = HttpsConnector::from((http, cfg));
        let client = builder.build(https);

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
}

impl Connector {
    pub fn new(bind: Bind, network: Network, resolver: Resolver, timeout: Duration) -> Self {
        let connect = Connect { bind, network, resolver };
        Self {
            connect: Arc::new(connect),
            timeout: timeout,
        }
    }
}

impl Service<Uri> for Connector {
    type Response = TcpStream;
    type Error    = Error;
    type Future   = Pin<Box<dyn Future<Output = Result<TcpStream, Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let connect = self.connect.clone();
        let timeout = self.timeout;
        Box::pin(connect.connect(dst, timeout))
    }
}

impl Connect {
    async fn connect(self: Arc<Self>, uri: Uri, expiry: Duration) -> Result<TcpStream> {
        let Self { bind, network, resolver } = &*self;

        let port = uri.port().as_ref().map(Port::as_u16);
        let port = match uri.scheme().map(Scheme::as_str) {
            Some("http")  => port.unwrap_or(80),
            Some("https") => port.unwrap_or(443),
            _             => return Err(anyhow!("{}: invalid scheme", uri)),
        };

        let addr = match uri.host() {
            Some(host) => resolver.lookup(host, *network).await?,
            None       => return Err(anyhow!("{}: missing host", uri)),
        };

        let connect = socket(&bind, (addr, port).into());

        Ok(timeout(expiry, connect).await??)
    }
}

async fn socket(bind: &Bind, addr: SocketAddr) -> Result<TcpStream> {
    let (domain, bind) = match addr {
        SocketAddr::V4(_) => (Domain::ipv4(), bind.sa4().into()),
        SocketAddr::V6(_) => (Domain::ipv6(), bind.sa6().into()),
    };

    let socket = Socket::new(domain, Type::stream(), Some(Protocol::tcp()))?;
    socket.bind(&bind)?;
    let stream = socket.into_tcp_stream();

    Ok(TcpStream::connect_std(stream, &addr).await?)
}
