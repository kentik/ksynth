use std::convert::{TryFrom, TryInto};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Error, Result};
use http::{HeaderMap, Method, header::HOST};
use http::response::Parts;
use http::uri::{self, Port, Uri};
use hyper::{self, body::Body};
use hyper::client::conn::Builder;
use tracing::{error, trace};
use netdiag::Bind;
use rustls::{ClientConfig, RootCertStore, ServerName};
use tokio_rustls::TlsConnector;
use crate::net::{Network, Resolver};
use crate::net::tls::{Identity, Verifier};
use super::stream::{socket, Connection, Peer};

#[derive(Clone)]
pub struct HttpClient {
    bind:     Bind,
    resolver: Resolver,
    tls:      TlsConnector,
    verifier: Arc<Verifier>,
}

#[derive(Debug)]
pub struct Request {
    method:  Method,
    uri:     Uri,
    scheme:  Scheme,
    host:    String,
    port:    u16,
    headers: HeaderMap,
    body:    Body,
    network: Network,
}

#[derive(Debug)]
pub struct Response {
    pub head:  Parts,
    pub body:  Body,
    pub peer:  Peer,
    pub times: Times,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Scheme {
    HTTP,
    HTTPS,
}

#[derive(Clone, Debug, Default)]
pub struct Times {
    pub dns: Duration,
    pub tcp: Duration,
    pub tls: Option<Duration>,
}

impl HttpClient {
    pub fn new(bind: Bind, resolver: Resolver, roots: RootCertStore) -> Result<Self> {
        let verifier = Arc::new(Verifier::new(roots));

        let mut config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        config.dangerous().set_certificate_verifier(verifier.clone());

        let tls = TlsConnector::from(Arc::new(config));

        Ok(Self { bind, resolver, tls, verifier })
    }

    pub async fn request(&self, request: Request) -> Result<Response> {
        let (conn, times) = self.connect(&request).await?;

        let http2 = conn.http2();
        let peer  = conn.peer();

        let (mut tx, connection) = Builder::new()
            .http2_only(http2)
            .handshake(conn)
            .await?;

        tokio::spawn(async move {
            match connection.await {
                Ok(()) => trace!("connection closed"),
                Err(e) => error!("connection failed: {}", e),
            }
        });

        let req = request.build(http2)?;
        let res = tx.send_request(req).await?;
        let (head, body) = res.into_parts();

        Ok(Response { head, body, peer, times })
    }

    async fn connect(&self, request: &Request) -> Result<(Connection, Times)> {
        let Request { scheme, ref host, port, network, .. } = *request;

        let mut times = Times::default();

        let start  = Instant::now();
        let addr   = self.resolver.lookup(host, network).await?;

        times.dns = start.elapsed();

        let start  = Instant::now();
        let addr   = SocketAddr::new(addr, port);
        let socket = socket(&self.bind, &addr).await?;
        let stream = socket.connect(addr).await?;

        times.tcp = start.elapsed();

        if scheme == Scheme::HTTP {
            let server = Identity::Unknown;
            let conn   = (stream, server).try_into()?;
            return Ok((conn, times))
        }

        let dnsname = ServerName::try_from(host.as_str())?;

        let start  = Instant::now();
        let stream = self.tls.connect(dnsname.clone(), stream).await?;

        times.tls = Some(start.elapsed());

        let (_, tls) = stream.get_ref();
        let certs  = tls.peer_certificates().unwrap_or_default();
        let server = self.verifier.verify(certs, &dnsname)?;
        let conn   = (stream, server).try_into()?;

        Ok((conn, times))
    }
}

impl Request {
    pub fn new(network: Network, method: Method, uri: Uri) -> Result<Self> {
        let scheme = uri.scheme_str().unwrap_or_default().parse()?;

        let host = uri.host().ok_or_else(|| {
            anyhow!("{}: missing host", uri)
        })?.to_owned();

        let port = uri.port().as_ref().map(Port::as_u16);
        let port = match scheme {
            Scheme::HTTP  => port.unwrap_or(80),
            Scheme::HTTPS => port.unwrap_or(443),
        };

        Ok(Self {
            method:  method,
            uri:     uri,
            scheme:  scheme,
            host:    host,
            port:    port,
            headers: HeaderMap::new(),
            body:    Body::empty(),
            network: network,
        })
    }

    pub fn headers(&mut self) -> &mut HeaderMap {
        &mut self.headers
    }

    pub fn body(&mut self) -> &mut Body {
        &mut self.body
    }

    fn build(self, http2: bool) -> Result<hyper::Request<Body>> {
        let mut uri     = self.uri;
        let mut headers = self.headers;

        if !http2 {
            let path = uri.into_parts().path_and_query;

            let mut parts = uri::Parts::default();
            parts.path_and_query = Some(match path {
                Some(path) if path != "/" => path,
                _                         => "/".parse()?,
            });
            uri = Uri::from_parts(parts)?;

            let Self { scheme, host, port, .. } = self;

            let host = match (scheme, port) {
                (Scheme::HTTP,   80) => host,
                (Scheme::HTTPS, 443) => host,
                _                    => format!("{}:{}", host, port),
            }.try_into()?;

            headers.entry(HOST).or_insert(host);
        }

        let mut request = hyper::Request::new(self.body);
        *request.method_mut()  = self.method;
        *request.headers_mut() = headers;
        *request.uri_mut()     = uri;

        Ok(request)
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
    use http::header::HeaderValue;
    use super::*;

    #[test]
    fn request_scheme() -> Result<()> {
        assert_eq!(Scheme::HTTP,  request("GET", "http://localhost")?.scheme);
        assert_eq!(Scheme::HTTPS, request("GET", "https://localhost")?.scheme);

        assert!(request("GET", "ssh://localhost").is_err());
        assert!(request("GET", "//localhost").is_err());

        Ok(())
    }

    #[test]
    fn request_port() -> Result<()> {
        assert_eq!(80,   request("GET", "http://localhost")?.port);
        assert_eq!(443,  request("GET", "https://localhost")?.port);

        assert_eq!(8000, request("GET", "http://localhost:8000")?.port);
        assert_eq!(4430, request("GET", "https://localhost:4430")?.port);

        Ok(())
    }

    #[test]
    fn build_http1() -> Result<()> {
        let host = HeaderValue::from_static("localhost");

        let req0 = request("GET", "http://localhost")?.build(false)?;

        assert_eq!(&Method::GET,         req0.method());
        assert_eq!(&Uri::from_str("/")?, req0.uri());
        assert_eq!(Some(&host),          req0.headers().get(HOST));

        let req0 = request("GET", "http://localhost/")?.build(false)?;
        let req1 = request("GET", "http://localhost/test")?.build(false)?;

        assert_eq!(&Uri::from_str("/")?,     req0.uri());
        assert_eq!(&Uri::from_str("/test")?, req1.uri());

        let req0 = request("GET", "http://localhost:81")?.build(false)?;
        let req1 = request("GET", "https://localhost:82")?.build(false)?;

        assert_eq!(Some(&"localhost:81".try_into()?), req0.headers().get(HOST));
        assert_eq!(Some(&"localhost:82".try_into()?), req1.headers().get(HOST));

        let mut req = request("GET", "http://example.com")?;
        req.headers().insert(HOST, host.clone());
        let req0 = req.build(false)?;

        assert_eq!(Some(&host), req0.headers().get(HOST));

        Ok(())
    }

    #[test]
    fn build_http2() -> Result<()> {
        let uri = Uri::from_str("http://localhost")?;
        let req = request("GET", &uri.to_string())?.build(true)?;

        assert_eq!(&Method::GET, req.method());
        assert_eq!(&uri,         req.uri());
        assert_eq!(None,         req.headers().get(HOST));

        Ok(())
    }

    fn request(method: &str, uri: &str) -> Result<Request> {
        Request::new(Network::Dual, method.parse()?, uri.parse()?)
    }
}
