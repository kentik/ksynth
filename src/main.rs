use std::process;
use anyhow::Error;
use clap::{load_yaml, App};
use env_logger::Builder;
use log::LevelFilter::{Info, Debug, Trace};
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

    let (module, level) = match args.occurrences_of("verbose") {
        0 => (Some(module_path!()), Info),
        1 => (Some(module_path!()), Debug),
        2 => (Some(module_path!()), Trace),
        _ => (None,                 Trace),
    };
    Builder::from_default_env().filter(module, level).init();

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
