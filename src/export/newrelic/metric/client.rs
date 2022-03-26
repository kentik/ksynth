use std::time::Duration;
use anyhow::{anyhow, Result};
use hyper::{Body, Client as HttpClient, Request, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use crate::export::Record;
use super::encode;

pub struct Client {
    client:   HttpClient<HttpsConnector<HttpConnector>>,
    agent:    String,
    endpoint: Uri,
    key:      HeaderValue,
}

impl Client {
    pub fn new(agent: String, key: &str) -> Result<Self> {
        let mut builder = HttpClient::builder();
        builder.pool_idle_timeout(Duration::from_secs(60));
        builder.pool_max_idle_per_host(1);

        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .enable_http2()
            .build();

        Ok(Self {
            client:   builder.build(https),
            agent:    agent,
            endpoint: ENDPOINT.parse()?,
            key:      HeaderValue::from_str(key)?,
        })
    }

    pub async fn send(&self, records: &[Record]) -> Result<()> {
        let mut vec = Vec::new();

        encode(&self.agent, records, &mut vec)?;

        let mut req = self.request()?;
        *req.body_mut() = vec.into();

        let res = self.client.request(req).await?;
        let status = res.status();

        if status != StatusCode::ACCEPTED {
            return Err(match status {
                StatusCode::FORBIDDEN => anyhow!("authentication"),
                status                => anyhow!("status {}", status),
            });
        }

        Ok(())
    }

    fn request(&self) -> Result<Request<Body>> {
        let content = HeaderValue::from_static("application/json");
        let body    = Body::from("");

        let mut req = Request::post(self.endpoint.clone()).body(body)?;
        req.headers_mut().insert(CONTENT_TYPE, content);
        req.headers_mut().insert("Api-Key", self.key.clone());

        Ok(req)
    }
}

const ENDPOINT: &str = "https://metric-api.newrelic.com/metric/v1";
