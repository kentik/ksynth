use std::process;
use anyhow::Error;
use clap::load_yaml;
use env_logger::Builder;
use log::LevelFilter::{Info, Debug, Trace};
use synag::{agent, args::Args, cmd};

fn main() {
    let yaml = load_yaml!("args.yml");
    let args = Args::new();
    let ver  = args.version();
    let args = args.matches(&yaml);

    let (module, level) = match args.occurrences_of("verbose") {
        0 => (Some(module_path!()), Info),
        1 => (Some(module_path!()), Debug),
        2 => (Some(module_path!()), Trace),
        _ => (None,                 Trace),
    };
    Builder::from_default_env().filter(module, level).init();

    match args.subcommand() {
        ("agent", Some(args)) => agent::agent(args, ver),
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
