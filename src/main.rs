use std::process;
use anyhow::Error;
use clap::{load_yaml, App};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry};
use ksynth::{agent::agent, cmd::*, args::Args, filter::Filter, version::Version};

fn main() -> Result<(), Error> {
    let version = Version::new();
    let yaml = load_yaml!("args.yml");
    let app  = App::from_yaml(yaml);
    let app  = app.version(&*version.version).long_version(&*version.detail);
    let args = app.get_matches();
    let args = Args::new(&args, yaml);

    let level = args.occurrences_of("verbose");
    let (filter, layer) = Filter::new(module_path!(), level)?;
    let format = fmt::layer().compact();
    registry().with(layer).with(format).init();

    match args.subcommand() {
        Some(("agent", args)) => agent(args, version, filter),
        Some(("knock", args)) => knock(args),
        Some(("ping",  args)) => ping(args),
        Some(("trace", args)) => trace(args),
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
