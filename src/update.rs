use std::time::Duration;
use std::thread::{self, JoinHandle};
use anyhow::Result;
use ed25519_compact::PublicKey;
use log::{debug, error, info};
use futures::future::{Abortable, Aborted, AbortHandle, AbortRegistration};
use tokio::runtime::{Builder, Runtime};
use tokio::time::{interval_at, Instant};
use notary::{Artifact, Client, Query, Updates};
use crate::version::Version;

pub struct Updater {
    primary: Runtime,
    runtime: Runtime,
    updates: Updates,
}

impl Updater {
    pub fn new(version: Version, release: bool, primary: Runtime) -> Result<Self> {
        let Version { name, version, arch, system, .. } = version;
        let version = normalize(&version);
        let query   = Query { name, version, arch, system, release };

        let public  = PublicKey::from_slice(&NOTARY_PUBLIC_KEY)?;
        let client  = Client::new(NOTARY_ENDPOINT.to_owned(), public)?;
        let updates = Updates::new(client, query);
        let runtime = Builder::new().basic_scheduler().enable_all().build()?;

        Ok(Self { primary, runtime, updates })
    }

    pub fn exec(self, enable: bool) -> (AbortHandle, JoinHandle<()>) {
        let (abort, registration) = AbortHandle::new_pair();
        let guard = thread::spawn(move || {
            match self.watch(enable, registration) {
                Ok(()) => debug!("updater finished"),
                Err(e) => error!("updater failed: {:?}", e),
            }
        });
        (abort, guard)
    }

    fn watch(self, enable: bool, registration: AbortRegistration) -> Result<()> {
        let Self { primary, mut runtime, updates } = self;

        let start  = Instant::now() + Duration::from_secs(60 * 2);
        let period = Duration::from_secs(60 * 60 * 24);

        let update = runtime.block_on(Abortable::new(async {
            let mut interval = interval_at(start, period);
            loop {
                let item = updates.watch(&mut interval).await;
                let Artifact { name, version, .. } = &item.artifact;

                info!("{} {} available", name, version);

                if enable {
                    match updates.fetch(item).await {
                        Ok(update) => return Ok(update),
                        Err(e)     => error!("{:?}", e),
                    }
                }
            }
        }, registration));

        let retry = Duration::from_secs(30);

        primary.shutdown_background();
        runtime.shutdown_background();

        match update {
            Ok(Ok(update)) => update.apply(retry),
            Ok(Err(e))     => Err(e),
            Err(Aborted)   => Ok(()),
        }
    }
}

fn normalize(version: &str) -> String {
    let mut split = version.split(|c: char| c.is_ascii_punctuation());
    let major = split.next().unwrap_or("0");
    let minor = split.next().unwrap_or("0");
    let patch = split.next().unwrap_or("0");
    format!("{}.{}.{}", major, minor, patch)
}

const NOTARY_ENDPOINT: &str = "https://notary.kentik.com/v1/latest";

const NOTARY_PUBLIC_KEY: &[u8] = &[
    81,  157,  82, 235, 221,  59,  74, 135,
    253,  58, 200, 226,  93,  81,  87,  25,
    219,  40,  44,  30,  23,  33, 191,  60,
    225,  27, 132,  81,  10, 212, 168,  56,
];
