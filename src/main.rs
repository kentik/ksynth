use std::process;
use anyhow::Error;
use clap::{load_yaml, App};
use env_logger::Builder;
use log::LevelFilter;
use synag::{agent, cmd};

fn main() {
    let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0");
    let version = option_env!("GIT_VERSION").unwrap_or(version).to_string();
    let commit  = option_env!("GIT_COMMIT").unwrap_or("<unknown>");
    let detail  = format!("{} ({})", version, commit);

    let yaml = load_yaml!("args.yml");
    let app  = App::from_yaml(yaml);
    let app  = app.version(&*version).long_version(&*detail);
    let args = app.get_matches();

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
        ("agent", Some(args)) => agent::agent(args, version),
        ("ping",  Some(args)) => cmd::ping(args),
        ("trace", Some(args)) => cmd::trace(args),
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
