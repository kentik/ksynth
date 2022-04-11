use std::process;
use anyhow::Error;
use clap::{self, load_yaml};
use tokio::runtime::Runtime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry};
use ksynth::args::{App, Args};
use ksynth::{agent::agent, cmd::*, filter::Filter, version::Version};

fn main() -> Result<(), Error> {
    let version = Version::new();

    let yaml = load_yaml!("args.yml");
    let app  = clap::App::from_yaml(yaml);
    let app  = app.version(&*version.version).long_version(&*version.detail);
    let args = app.get_matches();
    let args = Args::new(&args, yaml);

    let runtime = Runtime::new()?;

    let level = args.occurrences_of("verbose");
    let (filter, layer) = Filter::new(module_path!(), level)?;
    let format = fmt::layer().compact();
    registry().with(layer).with(format).init();

    let app = App { runtime, version, filter };

    match args.subcommand() {
        Some(("agent", args)) => agent(app, args),
        Some(("knock", args)) => knock(app, args),
        Some(("ping",  args)) => ping(app, args),
        Some(("trace", args)) => trace(app, args),
        _                     => unreachable!(),
    }.unwrap_or_else(abort);

    Ok(())
}

fn abort(e: Error) {
    match e.downcast_ref::<clap::Error>() {
        Some(e) => println!("{}", e.message),
        None    => panic!("{:?}", e),
    }
    process::exit(1);
}
