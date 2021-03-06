use std::collections::HashMap;
use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::net::SocketAddr;
use std::process;
use std::sync::Arc;
use anyhow::{Error, Result};
use clap::{value_t, values_t};
use ed25519_compact::{KeyPair, Seed};
use futures::FutureExt;
use log::{debug, error, info, warn};
use nix::{unistd::gethostname, sys::utsname::uname};
use rustls::RootCertStore;
use signal_hook::{iterator::Signals, {consts::signal::{SIGINT, SIGTERM, SIGUSR1}}};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::{LookupIpStrategy, ResolverConfig, ResolverOpts};
use trust_dns_resolver::system_conf::read_system_conf;
use synapi::{Client, Config as ClientConfig, Region};
use netdiag::Bind;
use crate::args::{App, Args};
use crate::cfg::Config;
use crate::ctl::Server;
use crate::exec::Factory;
use crate::export::Exporter;
use crate::net::{Listener, Network, Resolver, tls::TrustAnchors};
use crate::output::Output;
use crate::secure;
use crate::status::Monitor;
use crate::update::Updater;
use crate::watch::{Event, Watcher};

pub struct Agent {
    client: Arc<Client>,
    config: Config,
    keys:   KeyPair,
    events: Receiver<Event>,
    sender: Sender<Event>,
}

impl Agent {
    pub async fn exec(self, exporter: Exporter, factory: Factory) -> Result<()> {
        let Self { client, config, keys, events, sender } = self;

        let (tx, mut rx) = channel(16);

        let watcher = match config.watcher() {
            Some(watcher) => watcher.exec(sender).boxed(),
            None          => Watcher::new(client.clone(), keys, sender).exec().boxed(),
        };

        let executor = factory.executor(events, exporter.clone())?;
        let monitor  = Monitor::new(client, executor.status(), config)?;

        spawn(monitor.exec(),  tx.clone());
        spawn(watcher,         tx.clone());
        spawn(exporter.exec(), tx.clone());
        spawn(executor.exec(), tx.clone());

        match rx.recv().await {
            Some(e) => Err(e),
            None    => Ok(()),
        }
    }
}

fn spawn<T: Future<Output = Result<()>> + Send + 'static>(task: T, tx: Sender<Error>) {
    tokio::spawn(async move {
        match task.await {
            Ok(()) => Ok(()),
            Err(e) => tx.send(e).await
        }
    });
}

pub fn agent(app: App, args: Args<'_, '_>) -> Result<()> {
    let App { version, runtime, handles } = app;

    let id      = value_t!(args, "id", String)?;
    let name    = args.opt("name")?;
    let global  = args.is_present("global");
    let company = args.opt("company")?;
    let site    = args.opt("site")?;
    let regions = values_t!(args, "region", Region)?;
    let proxy   = args.opt("proxy")?;
    let ip4     = !args.is_present("ip6");
    let ip6     = !args.is_present("ip4");
    let user    = args.value_of("user");
    let update  = args.is_present("update");
    let listen  = args.opt("listen")?.unwrap_or_default();
    let output  = args.opt("output")?;
    let release = !args.is_present("rc");

    let mut bind = Bind::default();
    if let Some(addrs) = args.values_of("bind") {
        for addr in addrs {
            bind.set(addr.parse()?);
        }
    }

    let name = match name {
        Some(name) => name,
        None       => hostname()?,
    };

    let net = match (ip4, ip6) {
        (true, false) => Some(Network::IPv4),
        (false, true) => Some(Network::IPv6),
        _             => None,
    };

    info!("initializing {} {}", version.name, version.version);

    let keys = match fs::metadata(&id) {
        Ok(_)  => load(&id)?,
        Err(_) => init(&id)?,
    };

    let id = hex::encode(&keys.pk[..6]);
    debug!("name '{name}' identity: {id}");

    if let Err(e) = secure::apply(user) {
        error!("agent security failure: {e}");
    }

    let machine  = machine();
    let resolver = resolver(&bind, net)?;
    let roots    = trust_roots();
    let listener = runtime.block_on(Listener::new(listen));

    let config = Config {
        bind:     bind.clone(),
        listener: listener,
        network:  net,
        resolver: resolver,
        roots:    roots.clone(),
        tasks:    args.opt("config")?,
    };

    let factory = runtime.block_on(Factory::new(&config))?;

    let events = regions.into_iter().map(|region| {
        let name = name.clone();

        let client = Arc::new(Client::new(ClientConfig {
            name:    name.clone(),
            global:  global,
            region:  region.clone(),
            version: version.version.clone(),
            machine: machine.clone(),
            company: company,
            site:    site,
            proxy:   proxy.clone(),
            bind:    args.opt("bind")?,
            roots:   roots.clone(),
        })?);

        let exporter = match output.clone() {
            Some(Output::Influx(args))   => Exporter::influx(name, &config, args)?,
            Some(Output::NewRelic(args)) => Exporter::newrelic(name, &config, args)?,
            Some(Output::Kentik) | None  => Exporter::kentik(client.clone())?,
        };

        let factory  = factory.clone();
        let (tx, rx) = channel(128);

        let agent = Agent {
            client: client,
            keys:   keys,
            config: config.clone(),
            events: rx,
            sender: tx.clone(),
        };

        runtime.spawn(async move {
            if let Err(e) = agent.exec(exporter, factory).await {
                error!("agent failed: {e:?}");
                process::exit(1);
            }
        });

        Ok((region.name, tx))
    }).collect::<Result<HashMap<_, _>>>()?;

    let handle = runtime.handle().clone();
    let report = || {
        for report in events.values() {
            let (tx, mut rx) = channel(1);
            let report = report.clone();

            handle.spawn(async move {
                let request = Event::Report(tx);

                match report.send(request).await {
                    Ok(()) => info!("report requested"),
                    Err(e) => warn!("report error: {e:?}"),
                };

                if let Some(report) = rx.recv().await {
                    report.print();
                }
            });
        }
    };

    if let Some(socket) = args.opt("control")? {
        let report = events.clone();
        let server = Server::new(socket, handles, report);
        handle.spawn(async move {
            match server.exec().await {
                Ok(()) => debug!("control server finished"),
                Err(e) => error!("control server failed: {e:?}"),
            }
        });
    }

    let updater = Updater::new(version, release, runtime)?;
    let (abort, guard) = updater.exec(update);

    let mut signals = Signals::new(&[SIGINT, SIGTERM, SIGUSR1])?;
    for signal in signals.forever() {
        match signal {
            SIGINT | SIGTERM => break,
            SIGUSR1          => report(),
            _                => unreachable!(),
        }
    }

    abort.abort();
    guard.join().unwrap();

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
    let cstr = gethostname(&mut buf)?;
    Ok(cstr.to_string_lossy().to_string())
}

fn machine() -> String {
    let utsname = uname();

    let mut machine = String::new();
    machine.push_str(utsname.sysname());
    machine.push_str(" ");
    machine.push_str(utsname.nodename());
    machine.push_str(" ");
    machine.push_str(utsname.release());
    machine.push_str(" ");
    machine.push_str(utsname.version());
    machine.push_str(" ");
    machine.push_str(utsname.machine());

    machine
}

fn resolver(bind: &Bind, net: Option<Network>) -> Result<Resolver> {
    let (config, mut options) = read_system_conf().unwrap_or_else(|e| {
        warn!("resolver configuration error: {}", e);
        let config  = ResolverConfig::google();
        let options = ResolverOpts::default();
        (config, options)
    });

    let domain  = config.domain().cloned();
    let search  = config.search().to_vec();
    let servers = config.name_servers().iter().map(|server| {
        let mut server = server.clone();
        let local = server.socket_addr.ip().is_loopback();
        server.bind_addr = match server.socket_addr {
            SocketAddr::V4(_) if !local => Some(bind.sa4()),
            SocketAddr::V6(_) if !local => Some(bind.sa6()),
            _                           => None,
        };
        server
    }).collect::<Vec<_>>();

    let config = ResolverConfig::from_parts(domain, search, servers);

    options.ip_strategy = match net {
        Some(Network::IPv4)        => LookupIpStrategy::Ipv4Only,
        Some(Network::IPv6)        => LookupIpStrategy::Ipv6Only,
        Some(Network::Dual) | None => LookupIpStrategy::Ipv4AndIpv6,
    };

    Ok(Resolver::new(TokioAsyncResolver::tokio(config, options)?))
}

fn trust_roots() -> RootCertStore {
    let mut store = RootCertStore::empty();

    match TrustAnchors::native() {
        Ok(roots) => store.roots.extend_from_slice(&*roots),
        Err(e)    => warn!("invalid trust store: {}", e),
    };

    if store.roots.is_empty() {
        warn!("using static trust roots");
        let roots = TrustAnchors::webpki();
        store.roots.extend_from_slice(&*roots);
    }

    store
}
