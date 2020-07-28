use std::ffi::CStr;
use std::fs::{self, File};
use std::future::Future;
use std::io::{Error as IoError, Read};
use std::process;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{Error, Result};
use clap::{value_t, ArgMatches};
use ed25519_compact::{KeyPair, Seed};
use libc::gethostname;
use log::{debug, error, info};
use signal_hook::{iterator::Signals, SIGINT, SIGTERM};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Sender};
use synapi::{Client, Config};
use netdiag::Bind;
use crate::exec::{Executor, Network};
use crate::export::Exporter;
use crate::secure;
use crate::status::Monitor;
use crate::version::Version;
use crate::watch::Watcher;

pub struct Agent {
    client: Client,
    keys:   KeyPair,
}

impl Agent {
    pub fn new(client: Client, keys: KeyPair) -> Self {
        Self { client, keys }
    }

    pub async fn exec(self, bind: Bind, net: Network) -> Result<()> {
        let client = Arc::new(self.client);
        let keys   = self.keys;

        let (tx, mut rx) = channel(16);

        let (watcher, tasks) = Watcher::new(client.clone(), keys);
        let exporter = Arc::new(Exporter::new(client.clone()));
        let executor = Executor::new(tasks, exporter.clone(), bind, net).await?;
        let monitor  = Monitor::new(client, executor.status());

        spawn(monitor.exec(),  tx.clone());
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

pub fn agent(args: &ArgMatches, version: Version) -> Result<()> {
    let id      = value_t!(args, "id", String)?;
    let name    = args.value_of("name");
    let global  = args.is_present("global");
    let company = args.value_of("company");
    let region  = value_t!(args, "region", String)?;
    let proxy   = args.value_of("proxy");
    let ip4     = !args.is_present("ip6");
    let ip6     = !args.is_present("ip4");
    let port    = value_t!(args, "port", u32)?;
    let user    = args.value_of("user");

    let mut bind = Bind::default();
    if let Some(addrs) = args.values_of("bind") {
        for addr in addrs {
            bind.set(addr.parse()?);
        }
    }

    let name = match name {
        Some(str) => str.to_owned(),
        None      => hostname()?,
    };

    let company = company.map(u64::from_str).transpose()?;

    let net = Network {
        ip4: ip4,
        ip6: ip6,
        set: !ip4 || !ip6,
    };

    info!("initializing {} {}", version.name, version.version);

    let keys = match fs::metadata(&id) {
        Ok(_)  => load(&id)?,
        Err(_) => init(&id)?,
    };

    let id = hex::encode(&keys.pk[..6]);
    debug!("name '{}' identity: {}", name, id);

    if let Err(e) = secure::apply(user) {
        error!("agent security failure: {}", e);
    }

    let client = Client::new(Config {
        name:    name,
        global:  global,
        region:  region,
        version: version.version,
        company: company,
        proxy:   proxy.map(String::from),
        port:    port,
        bind:    args.value_of("bind").map(String::from), // Only used for passing back to api to get local ip.
    })?;

    let runtime = Runtime::new()?;
    let agent   = Agent::new(client, keys);

    runtime.spawn(async move {
        if let Err(e) = agent.exec(bind, net).await {
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

fn load(path: &str) -> Result<KeyPair> {
    let mut file  = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(KeyPair::from_slice(&bytes)?)
}

fn init(path: &str) -> Result<KeyPair> {
    info!("generating new identity");
    let seed = Seed::generate();
    let keys  = KeyPair::from_seed(seed);
    fs::write(path, &keys[..])?;
    Ok(keys)
}

fn hostname() -> Result<String> {
    let mut buf = [0u8; 256];
    Ok(unsafe {
        let ptr = buf.as_mut_ptr() as *mut _;
        let len = buf.len();
        match gethostname(ptr, len) {
            0 => CStr::from_ptr(ptr).to_string_lossy(),
            _ => Err(IoError::last_os_error())?,
        }
    }.to_string())
}
