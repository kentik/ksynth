use std::io::prelude::*;
use std::str;
use std::time::Duration;
use ed25519_compact::{PublicKey, Signature};
use log::error;
use reqwest::{Client as HttpClient};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio_stream::StreamExt;
use super::artifact::Artifact;
use super::error::Error;
use super::expand::Expand;

pub struct Client {
    client:   HttpClient,
    endpoint: String,
    public:   PublicKey,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Query {
    pub name:    String,
    pub version: String,
    pub arch:    String,
    pub system:  String,
    pub release: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Item {
    pub artifact: Artifact,
    pub location: String,
}

#[serde(untagged)]
#[derive(Debug, Serialize, Deserialize)]
pub enum Response<T> {
    Success(T),
    Failure(Failure),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Failure {
    error: String,
}

impl Client {
    pub fn new(endpoint: String, public: PublicKey) -> Result<Self, Error> {
        let mut client = HttpClient::builder();
        client = client.timeout(Duration::from_secs(60 * 5));

        Ok(Self {
            client:   client.build()?,
            endpoint: endpoint,
            public:   public,
        })
    }

    pub async fn latest(&self, query: &Query) -> Result<Option<Item>, Error> {
        self.send(&self.endpoint, query).await
    }

    pub async fn stream(&self, item: &Item, sink: impl Write) -> Result<(), Error> {
        let r = self.client.get(&item.location).send().await?;

        let status = r.status();
        if !status.is_success() {
            return Err(Error::Status(status, String::new()));
        }

        let signature  = Signature::from_slice(&item.artifact.signature)?;
        let mut expand = Expand::new(sink)?;
        let mut stream = r.bytes_stream();

        while let Some(chunk) = stream.next().await {
            expand.update(&chunk?)?;
        }

        let hash = expand.finish()?;
        self.public.verify(&hash[..], &signature)?;

        Ok(())
    }

    async fn send<T: Serialize, U: DeserializeOwned>(&self, url: &str, body: &T) -> Result<U, Error> {
        let r = self.client.post(url).json(body).send().await?;

        let status = r.status();
        let body   = r.bytes().await?;

        if !status.is_success() {
            let body = str::from_utf8(&body).unwrap_or("<invalid>");
            return Err(Error::Status(status, body.to_owned()));
        }

        match json(&body)? {
            Response::Success(v) => Ok(v),
            Response::Failure(f) => Err(Error::Application(f.error)),
        }
    }
}

fn json<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|e| {
        let json = str::from_utf8(bytes).unwrap_or("<invalid>");
        error!("{:?}: {}", e, json);
        Error::Transport(e.to_string())
    })
}
