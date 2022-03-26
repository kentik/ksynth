use std::time::Duration;
use anyhow::{anyhow, Result};
use hyper::{Body, Client as HttpClient, Request, StatusCode, Uri};
use hyper::client::HttpConnector;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use crate::export::Record;
use crate::output::Args;
use super::encode;

pub struct Client {
    client:   HttpClient<HttpsConnector<HttpConnector>>,
    agent:    String,
    endpoint: Uri,
    key:      HeaderValue,
}

impl Client {
    pub fn new(agent: String, args: Args) -> Result<Self> {
        let account = args.get("account")?;
        let key     = args.get("key")?;
        let region  = args.opt("region").unwrap_or("US");

        let host = match region.to_ascii_uppercase().as_str() {
            "US" => "insights-collector.newrelic.com",
            "EU" => "insights-collector.eu01.nr-data.net",
            _    => return Err(anyhow!("invalid region: {}", region)),
        };

        let endpoint = format!("https://{}/v1/accounts/{}/events", host, account);

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
            endpoint: endpoint.parse()?,
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

        if status != StatusCode::OK {
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
        req.headers_mut().insert("X-Insert-Key", self.key.clone());

        Ok(req)
    }
}
