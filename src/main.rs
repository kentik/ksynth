use std::fs::{self, File};
use std::io::Read;
use std::process;
use anyhow::Result;
use clap::{App, load_yaml, value_t};
use ed25519_dalek::Keypair;
use env_logger::Builder;
use log::{error, info, LevelFilter::{Info, Debug, Trace}};
use rand::thread_rng;
use signal_hook::{iterator::Signals, SIGINT, SIGTERM};
use tokio::runtime::Runtime;
use synapi::Client;
use synag::Agent;

fn main() -> Result<()> {
    let yaml = load_yaml!("args.yml");
    let name = env!("CARGO_PKG_NAME");
    let ver  = env!("CARGO_PKG_VERSION");
    let args = App::from_yaml(&yaml).version(ver).get_matches();

    let id     = value_t!(args, "id", String)?;
    let region = value_t!(args, "region", String)?;
    let proxy  = args.value_of("proxy");

    let (module, level) = match args.occurrences_of("verbose") {
        0 => (Some(module_path!()), Info),
        1 => (Some(module_path!()), Debug),
        2 => (Some(module_path!()), Trace),
        _ => (None,                 Trace),
    };
    Builder::from_default_env().filter(module, level).init();

    info!("initializing {} {}", name, ver);

    let keys = match fs::metadata(&id) {
        Ok(_)  => load(&id)?,
        Err(_) => init(&id)?,
    };

    let client  = Client::new(region.as_ref(), proxy)?;
    let runtime = Runtime::new()?;
    let agent   = Agent::new(client, keys);

    runtime.spawn(async move {
        if let Err(e) = agent.exec().await {
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
