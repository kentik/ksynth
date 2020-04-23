use std::time::Duration;
use async_compression::futures::write::GzipEncoder;
use ed25519_dalek::Keypair;
use futures::io::AsyncWriteExt;
use reqwest::{Client as HttpClient, Proxy};
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use time::get_time;
use crate::{Error, error::Backend};
use crate::auth::Auth;
use crate::tasks::Tasks;

#[derive(Clone)]
pub struct Client {
    client: HttpClient,
    auth:   String,
    tasks:  String,
    submit: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Response<T> {
    Success(T),
    Failure(Failure),
}

#[derive(Debug, Deserialize)]
struct Failure {
    status: u32,
    msg:    String,
}

impl Client {
    pub fn new(region: &str, proxy: Option<&str>) -> Result<Self, Error> {
        let domain = match region.to_ascii_uppercase().as_ref() {
            "US" => "kentik.com".to_owned(),
            "EU" => "kentik.eu".to_owned(),
            name => format!("{}.kentik.com", name.to_ascii_lowercase()),
        };

        let mut client = HttpClient::builder();
        client = client.timeout(Duration::from_secs(30));

        if let Some(proxy) = proxy.map(Proxy::all) {
            client = client.proxy(proxy?);
        }

        Ok(Self {
            client: client.build()?,
            auth:   format!("https://portal.{}:8888/v1/syn/auth",  domain),
            tasks:  format!("https://portal.{}:8888/v1/syn/tasks", domain),
            submit: format!("https://flow.{}/chf",                 domain),
        })
    }

    pub async fn auth<'a>(&self, keys: &Keypair, version: &'a str) -> Result<Auth, Error> {
        #[derive(Debug, Serialize)]
        struct Request<'a> {
            agent:     String,
            version:   &'a str,
            timestamp: String,
            signature: String,
        }

        let key = &keys.public;
        let now = get_time().sec.to_string();
        let sig = keys.sign(now.as_bytes());

        Ok(self.send(&self.auth, &Request {
            agent:     hex::encode(&key.to_bytes()[..]),
            version:   version,
            timestamp: now,
            signature: hex::encode(&sig.to_bytes()[..]),
        }).await?)
    }

    pub async fn tasks<'a>(&self, session: &'a str, since: u64) -> Result<Tasks, Error> {
        #[derive(Debug, Serialize)]
        struct Request<'a> {
            session: &'a str,
            since:   u64,
        };

        Ok(self.send(&self.tasks, &Request {
            session: session,
            since:   since,
        }).await?)
    }

    pub async fn export(&self, sid: &str, email: &str, token: &str, flow: &[u8]) -> Result<(), Error> {
        let url = format!("{}?sid=0&sender_id={}", self.submit, sid);

        let mut e = GzipEncoder::new(Vec::new());
        e.write_all(flow).await?;
        e.close().await?;
        let flow = e.into_inner();

        let req = self.client.post(&url)
            .header(CONTENT_TYPE, "application/binary")
            .header(CONTENT_ENCODING, "gzip")
            .header(AUTH_EMAIL, email)
            .header(AUTH_TOKEN, token)
            .body(flow)
            .build()?;

        match self.client.execute(req).await?.status() {
            s if s.is_success() => Ok(()),
            s                   => Err(s.into()),
        }
    }

    async fn send<T: Serialize, U: DeserializeOwned>(&self, url: &str, body: &T) -> Result<U, Error> {
        let r = self.client.post(url).json(body).send().await?;

        if !r.status().is_success() {
            return Err(r.json::<Backend>().await?.into());
        }

        match r.json().await? {
            Response::Success(v) => Ok(v),
            Response::Failure(f) => Err(f.into()),
        }
    }
}

impl From<Failure> for Error {
    fn from(Failure { status, msg }: Failure) -> Self {
        Error::Application(status, msg)
    }
}

const AUTH_EMAIL: &str = "X-CH-Auth-Email";
const AUTH_TOKEN: &str = "X-CH-Auth-API-Token";
