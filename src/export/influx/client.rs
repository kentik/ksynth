use std::io::{Read, Write};
use std::time::Duration;
use anyhow::{anyhow, Result};
use base64::{STANDARD, write::EncoderStringWriter};
use hyper::{Client as HttpClient, Method, Request};
use hyper::body::{aggregate, Buf};
use hyper::client::HttpConnector;
use hyper::header::{AUTHORIZATION, HeaderValue};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use rustls::ClientConfig;

pub struct Client {
    client:   HttpClient<HttpsConnector<HttpConnector>>,
    endpoint: String,
    auth:     Auth
}

#[derive(Debug, Eq, PartialEq)]
pub enum Auth {
    Basic(String, String),
    Token(String),
    None,
}

impl Client {
    pub fn new(endpoint: &str, cfg: ClientConfig, auth: Auth) -> Result<Self> {
        let mut builder = HttpClient::builder();
        builder.pool_idle_timeout(Duration::from_secs(60));

        let https = HttpsConnectorBuilder::new()
            .with_tls_config(cfg)
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();

        Ok(Self {
            client:   builder.build(https),
            endpoint: endpoint.to_owned(),
            auth:     auth,
        })
    }

    pub async fn send(&self, body: &[u8]) -> Result<()> {
        let mut req = Request::builder()
            .method(Method::POST)
            .uri(&self.endpoint)
            .body(body.to_vec().into())?;

        if let Auth::Token(token) = &self.auth {
            let value = format!("Token {}", token);
            let value = HeaderValue::from_str(&value)?;
            req.headers_mut().insert(AUTHORIZATION, value);
        } else if let Auth::Basic(username, password) = &self.auth {
            let mut buf = "Basic ".to_string();
            let mut enc = EncoderStringWriter::from(&mut buf, STANDARD);
            write!(enc, "{}:{}", username, password)?;

            let value = HeaderValue::from_str(&enc.into_inner())?;
            req.headers_mut().insert(AUTHORIZATION, value);
        }

        let res = self.client.request(req).await?;
        let status = res.status();

        if !status.is_success() {
            let mut body = aggregate(res).await?.reader();
            let mut msg  = String::new();
            body.read_to_string(&mut msg)?;
            return Err(anyhow!("{}: {}", status, msg));
        }

        Ok(())
    }
}
