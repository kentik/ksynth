use std::io::Read;
use std::time::Duration;
use anyhow::{anyhow, Result};
use hyper::{Client as HttpClient, Method, Request};
use hyper::body::{aggregate, Buf};
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;

pub struct Client {
    client:   HttpClient<HttpsConnector<HttpConnector>>,
    endpoint: String,
}

impl Client {
    pub fn new(endpoint: &str) -> Result<Self> {
        let mut builder = HttpClient::builder();
        builder.pool_idle_timeout(Duration::from_secs(60));

        let https = HttpsConnector::with_native_roots();

        Ok(Self {
            client:   builder.build(https),
            endpoint: endpoint.to_owned(),
        })
    }

    pub async fn send(&self, body: &[u8]) -> Result<()> {
        let req = Request::builder()
            .method(Method::POST)
            .uri(&self.endpoint)
            .body(body.to_vec().into())?;

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
