use std::process;
use anyhow::Error;
use clap::{App, load_yaml};
use env_logger::Builder;
use log::LevelFilter::{Info, Debug, Trace};
use synag::{agent, cmd};

fn main() {
    let ver  = env!("CARGO_PKG_VERSION");
    let yaml = load_yaml!("args.yml");
    let args = App::from_yaml(&yaml).version(ver).get_matches();

    let (module, level) = match args.occurrences_of("verbose") {
        0 => (Some(module_path!()), Info),
        1 => (Some(module_path!()), Debug),
        2 => (Some(module_path!()), Trace),
        _ => (None,                 Trace),
    };
    Builder::from_default_env().filter(module, level).init();

    match args.subcommand() {
        ("agent", Some(args)) => agent::agent(args),
        ("ping",  Some(args)) => cmd::ping(args),
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
