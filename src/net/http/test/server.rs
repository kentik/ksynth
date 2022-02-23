use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::{Error, Result};
use hyper::{Body, Request, Response};
use hyper::server::conn::Http;
use hyper::service::service_fn;
use log::{debug, error};
use rcgen::{generate_simple_self_signed, Certificate};
use rustls::{self, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

pub struct Server {
    pub http:  SocketAddr,
    pub https: SocketAddr,
    pub roots: RootCertStore,
}

pub async fn server(bind: SocketAddr, alpn: &[Vec<u8>]) -> Result<Server> {
    let subjects = vec!["localhost".to_string()];
    let cert = generate_simple_self_signed(subjects)?;

    let bytes     = &[cert.serialize_der()?];
    let mut roots = RootCertStore::empty();
    roots.add_parsable_certificates(bytes);

    let accept  = TcpListener::bind(bind).await?;
    let http    = accept.local_addr()?;
    tokio::spawn(tcp(accept));

    let accept  = TcpListener::bind(bind).await?;
    let https   = accept.local_addr()?;
    tokio::spawn(tls(accept, cert, alpn.to_vec()));

    Ok(Server { http, https, roots })
}

async fn tcp(tcp: TcpListener) -> Result<()> {
    loop {
        let (stream, _) = tcp.accept().await?;

        tokio::spawn(async move {
            let http = Http::new();
            let conn = http.serve_connection(stream, service_fn(index));
            match conn.await {
                Ok(()) => debug!("connection finished"),
                Err(e) => error!("connection error: {}", e),
            }
        });
    }
}

async fn tls(tcp: TcpListener, cert: Certificate, alpn: Vec<Vec<u8>>) -> Result<()> {
    let mut keys = Cursor::new(cert.serialize_private_key_pem());
    let mut cert = Cursor::new(cert.serialize_pem()?);

    let key  = pkcs8_private_keys(&mut keys).unwrap_or_default();
    let cert = certs(&mut cert).unwrap_or_default();

    let key  = rustls::PrivateKey(key[0].clone());
    let cert = vec![rustls::Certificate(cert[0].clone())];

    let mut cfg = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert, key)?;
    cfg.alpn_protocols = alpn;

    let tls = TlsAcceptor::from(Arc::new(cfg));

    loop {
        let (stream, _) = tcp.accept().await?;
        let stream = tls.accept(stream).await?;

        tokio::spawn(async move {
            let http = Http::new();
            let conn = http.serve_connection(stream, service_fn(index));
            match conn.await {
                Ok(()) => debug!("connection finished"),
                Err(e) => error!("connection error: {}", e),
            }
        });
    }
}

async fn index(_: Request<Body>) -> Result<Response<Body>, Error> {
    Ok(Response::new(Body::from("ok")))
}
