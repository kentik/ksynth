use std::str;
use std::time::Duration;
use anyhow::{anyhow, Result};
use hyper::body::HttpBody;
use tokio::runtime::Handle;
use tokio::time::timeout;
use v8vm::vm::Resolver;
use v8vm::ex::fetch::{Client, Request, Response};
use crate::net::Network;
use crate::net::http::{self, HttpClient};

pub struct FetchClient {
    client: HttpClient,
    handle: Handle,
}

impl FetchClient {
    pub fn new(client: HttpClient, handle: Handle) -> Self {
        Self { client, handle }
    }

    async fn send(client: HttpClient, request: Request) -> Result<Response> {
        let network = Network::Dual;
        let method  = request.method.parse()?;
        let url     = request.url.parse()?;

        let request  = http::Request::new(network, method, url)?;
        let mut res  = client.request(request).await?;
        let status   = res.head.status;
        let mut body = String::new();

        while let Some(chunk) = res.body.data().await {
            let chunk = chunk?;
            let chunk = str::from_utf8(&chunk)?;
            body.push_str(chunk);
        }

        Ok(Response {
            status: status,
            body:   body,
        })
    }
}

impl Client for FetchClient {
    fn fetch(&self, request: Request, resolver: Resolver) {
        let client = self.client.clone();
        self.handle.spawn(async move {
            let expiry = Duration::from_secs(10);

            let result = Self::send(client, request);

            match timeout(expiry, result).await {
                Ok(Ok(r))  => resolver.resolve(Box::new(r)),
                Ok(Err(e)) => resolver.reject(Box::new(e)),
                Err(_)     => resolver.reject(Box::new(anyhow!("timeout"))),
            }
        });
    }
}
