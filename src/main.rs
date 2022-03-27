use std::process;
use anyhow::Error;
use clap::{load_yaml, App};
use env_logger::Builder;
use log::LevelFilter;
use ksynth::{agent, cmd, args::Args, version::Version};

fn main() {
    let version = Version::new();
    let yaml = load_yaml!("args.yml");
    let app  = App::from_yaml(yaml);
    let app  = app.version(&*version.version).long_version(&*version.detail);
    let args = app.get_matches();
    let args = Args::new(&args, yaml);

    let mut builder = Builder::from_default_env();
    let mut filter  = |agent, other| {
        builder.filter(Some(module_path!()), agent);
        builder.filter(None,                 other);
    };

    match args.occurrences_of("verbose") {
        0 => filter(LevelFilter::Info,  LevelFilter::Warn),
        1 => filter(LevelFilter::Debug, LevelFilter::Info),
        2 => filter(LevelFilter::Trace, LevelFilter::Info),
        3 => filter(LevelFilter::Trace, LevelFilter::Debug),
        _ => filter(LevelFilter::Trace, LevelFilter::Trace),
    };

    builder.init();

    match args.subcommand() {
        Some(("agent", args)) => agent::agent(args, version),
        Some(("knock", args)) => cmd::knock(args),
        Some(("ping",  args)) => cmd::ping(args),
        Some(("trace", args)) => cmd::trace(args),
        _                     => unreachable!(),
    }.unwrap_or_else(abort);
}

fn abort(e: Error) {
    match e.downcast_ref::<clap::Error>() {
        Some(e) => println!("{}", e.message),
        None    => panic!("{:?}", e),
    }
    process::exit(1);
}
