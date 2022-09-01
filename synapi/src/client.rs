use std::mem;
use std::net::IpAddr;
use std::str;
use std::sync::Arc;
use std::time::Duration;
use async_compression::futures::write::GzipEncoder;
use ed25519_compact::KeyPair;
use futures::io::AsyncWriteExt;
use log::error;
use proxy_env::reqwest::client_builder;
use reqwest::{Client as HttpClient, Proxy};
use reqwest::header::{CONTENT_ENCODING, CONTENT_TYPE};
use rustls::{ClientConfig, RootCertStore};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use crate::{Error, error::{Application, Backend}};
use crate::auth::Auth;
use crate::config::Config;
use crate::{okay::Okay, status::Report, tasks::Tasks};

#[derive(Debug)]
pub struct Client {
    client:  HttpClient,
    config:  Config,
    auth:    String,
    tasks:   String,
    status:  String,
    submit:  String,
    session: RwLock<Session>,
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
    retry:  u64,
}

impl Client {
    pub fn new(mut config: Config) -> Result<Self, Error> {
        let mut roots = RootCertStore::empty();
        mem::swap(&mut roots, &mut config.roots);

        let Config { region, proxy, bind, .. } = &config;

        let mut cfg = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(roots)
            .with_no_client_auth();
        cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let mut client = match proxy {
            Some(proxy) => HttpClient::builder().proxy(Proxy::all(proxy)?),
            None        => client_builder(),
        };

        client = client.timeout(Duration::from_secs(30));
        client = client.use_preconfigured_tls(cfg);

        if let Some(addr) = bind {
            client = client.local_address(*addr);
        }

        let auth   = format!("{}/auth",   region.api);
        let tasks  = format!("{}/tasks",  region.api);
        let status = format!("{}/status", region.api);
        let submit = region.flow.clone();

        Ok(Self {
            client:  client.build()?,
            config:  config,
            auth:    auth,
            tasks:   tasks,
            status:  status,
            submit:  submit,
            session: RwLock::new(Session::None),
        })
    }

    pub async fn auth(&self, keys: &KeyPair) -> Result<Auth, Error> {
        #[derive(Debug, Serialize)]
        struct Request<'a> {
            agent:      String,
            company_id: Option<u64>,
            site:       Option<u64>,
            version:    &'a str,
            timestamp:  String,
            signature:  String,
            name:       &'a str,
            global:     bool,
            os:         &'a str,
            bind:       Option<&'a IpAddr>,
        }

        let key = &keys.pk;
        let now = OffsetDateTime::now_utc().unix_timestamp().to_string();
        let sig = keys.sk.sign(now.as_bytes(), None);

        let auth = self.send(&self.auth, &Request {
            agent:      hex::encode(&key[..]),
            company_id: self.config.company,
            site:       self.config.site,
            version:    &self.config.version,
            timestamp:  now,
            signature:  hex::encode(&sig[..]),
            name:       &self.config.name,
            global:     self.config.global,
            os:         &self.config.machine,
            bind:       self.config.bind.as_ref(),
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
        }

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
        }

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

        let status = r.status();
        let body   = r.bytes().await?;

        if !status.is_success() {
            return Err(json::<Backend>(&body)?.into());
        }

        match json(&body)? {
            Response::Success(v) => Ok(v),
            Response::Failure(f) => Err(f.into()),
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

impl From<Failure> for Error {
    fn from(Failure { status, msg, retry }: Failure) -> Self {
        Error::Application(Application {
            code:    status,
            message: msg,
            retry:   match retry {
                0 => None,
                n => Some(Duration::from_millis(n)),
            }
        })
    }
}

const AUTH_EMAIL: &str = "X-CH-Auth-Email";
const AUTH_TOKEN: &str = "X-CH-Auth-API-Token";
