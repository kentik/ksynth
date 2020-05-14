use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::process;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{Error, Result};
use clap::{value_t, ArgMatches};
use ed25519_dalek::Keypair;
use log::{error, info};
use rand::thread_rng;
use signal_hook::{iterator::Signals, SIGINT, SIGTERM};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Sender};
use synapi::{Client, Config};
use crate::exec::Executor;
use crate::export::Exporter;
use crate::watch::Watcher;

pub struct Agent {
    client: Arc<Client>,
    keys:   Keypair,
}

impl Agent {
    pub fn new(client: Arc<Client>, keys: Keypair) -> Self {
        Self { client, keys }
    }

    pub async fn exec(self, ip4: bool, ip6: bool) -> Result<()> {
        let client = self.client;
        let keys   = self.keys;

        let (tx, mut rx) = channel(16);

        let (watcher, tasks) = Watcher::new(client.clone(), keys);
        let exporter = Arc::new(Exporter::new(client.clone()));
        let executor = Executor::new(tasks, exporter.clone(), ip4, ip6).await?;

        spawn(watcher.exec(),  tx.clone());
        spawn(exporter.exec(), tx.clone());
        spawn(executor.exec(), tx.clone());

        match rx.recv().await {
            Some(e) => Err(e),
            None    => Ok(()),
        }
    }
}

fn spawn<T: Future<Output = Result<()>> + Send + 'static>(task: T, mut tx: Sender<Error>) {
    tokio::spawn(async move {
        match task.await {
            Ok(()) => Ok(()),
            Err(e) => tx.send(e).await
        }
    });
}

pub fn agent(args: &ArgMatches, version: String) -> Result<()> {
    let name = env!("CARGO_PKG_NAME");

    let id      = value_t!(args, "id", String)?;
    let company = args.value_of("company");
    let region  = value_t!(args, "region", String)?;
    let proxy   = args.value_of("proxy");
    let ip4     = !args.is_present("ip6");
    let ip6     = !args.is_present("ip4");

    let company = company.map(u64::from_str).transpose()?;

    info!("initializing {} {}", name, version);

    let keys = match fs::metadata(&id) {
        Ok(_)  => load(&id)?,
        Err(_) => init(&id)?,
    };

    let config = Config {
        region:  region,
        version: version,
        company: company,
        proxy:   proxy.map(String::from),
    };

    let client  = Arc::new(Client::new(config)?);
    let runtime = Runtime::new()?;
    let agent   = Agent::new(client, keys);

    runtime.spawn(async move {
        if let Err(e) = agent.exec(ip4, ip6).await {
            error!("agent failed: {:?}", e);
            process::exit(1);
        }
    });

    let signals = Signals::new(&[SIGINT, SIGTERM])?;
    for signal in signals.forever() {
        match signal {
            SIGINT | SIGTERM => break,
            _                => unreachable!(),
        }
    }

    drop(runtime);

    Ok(())
}

fn load(path: &str) -> Result<Keypair> {
    let mut file  = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Keypair::from_bytes(&bytes)?)
}

fn init(path: &str) -> Result<Keypair> {
    info!("generating new identity");
    let mut rng = thread_rng();
    let keys  = Keypair::generate(&mut rng);
    let bytes = keys.to_bytes();
    fs::write(path, &bytes[..])?;
    Ok(keys)
}
