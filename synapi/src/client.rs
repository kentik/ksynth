use std::sync::Arc;
use std::time::Duration;
use async_compression::futures::write::GzipEncoder;
use ed25519_dalek::Keypair;
use futures::io::AsyncWriteExt;
use reqwest::{Client as HttpClient, Proxy};
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use time::get_time;
use tokio::sync::RwLock;
use crate::{Error, error::Backend};
use crate::auth::Auth;
use crate::config::Config;
use crate::{okay::Okay, status::Report, tasks::Tasks};

#[derive(Debug)]
pub struct Client {
    client:  HttpClient,
    company: Option<u64>,
    version: String,
    session: RwLock<Session>,
    auth:    String,
    tasks:   String,
    status:  String,
    submit:  String,
}

#[derive(Debug)]
enum Session {
    Some(Arc<String>),
    None,
}

#[derive(Debug)]
pub enum Response<T> {
    Success(T),
    Failure(Failure),
}

#[derive(Debug, Deserialize)]
pub struct Failure {
    status: u32,
    msg:    String,
}

impl Client {
    pub fn new(config: Config) -> Result<Self, Error> {
        let Config { region, version, company, proxy } = config;
        let domain = match region.to_ascii_uppercase().as_ref() {
            "US" => "kentik.com".to_owned(),
            "EU" => "kentik.eu".to_owned(),
            name => format!("{}.kentik.com", name.to_ascii_lowercase()),
        };

        let mut client = HttpClient::builder();
        client = client.timeout(Duration::from_secs(30));

        if let Some(proxy) = proxy.as_ref().map(Proxy::all) {
            client = client.proxy(proxy?);
        }

        Ok(Self {
            client:  client.build()?,
            company: company,
            version: version,
            session: RwLock::new(Session::None),
            auth:    format!("https://api.{}/api/agent/v1/syn/auth",   domain),
            tasks:   format!("https://api.{}/api/agent/v1/syn/tasks",  domain),
            status:  format!("https://api.{}/api/agent/v1/syn/status", domain),
            submit:  format!("https://flow.{}/chf",                  domain),
        })
    }

    pub async fn auth(&self, keys: &Keypair) -> Result<Auth, Error> {
        #[derive(Debug, Serialize)]
        struct Request<'a> {
            agent:      String,
            company_id: Option<String>,
            version:    &'a str,
            timestamp:  String,
            signature:  String,
        }

        let company = self.company.as_ref().map(u64::to_string);

        let key = &keys.public;
        let now = get_time().sec.to_string();
        let sig = keys.sign(now.as_bytes());

        let auth = self.send(&self.auth, &Request {
            agent:      hex::encode(&key.to_bytes()[..]),
            company_id: company,
            version:    &self.version,
            timestamp:  now,
            signature:  hex::encode(&sig.to_bytes()[..]),
        }).await?;

        if let Auth::Ok((_, session)) = &auth {
            let session = Arc::new(session.to_owned());
            let mut lock = self.session.write().await;
            *lock = Session::Some(session);
        }

        Ok(auth)
    }

    pub async fn tasks(&self, since: u64) -> Result<Tasks, Error> {
        #[derive(Serialize)]
        struct Request<'a> {
            session: &'a str,
            since:   u64,
        };

        let session = self.session().await?;

        Ok(self.send(&self.tasks, &Request {
            session: &session,
            since:   since,
        }).await?)
    }

    pub async fn status(&self, report: &Report) -> Result<Okay, Error> {
        #[derive(Serialize)]
        struct Request<'a> {
            session: &'a str,
            report:  &'a Report,
        };

        let session = self.session().await?;

        Ok(self.send(&self.status, &Request {
            session: &session,
            report:  report,
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

    async fn session(&self) -> Result<Arc<String>, Error> {
        match &*(self.session.read().await) {
            Session::Some(session) => Ok(session.clone()),
            Session::None          => Err(Error::Session),
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
