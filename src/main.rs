use std::collections::HashMap;
use std::env::{var, VarError};
use std::process;
use anyhow::Error;
use clap::{self, load_yaml};
use opentelemetry::KeyValue;
use opentelemetry::runtime::Tokio;
use opentelemetry::sdk::Resource;
use opentelemetry::sdk::trace::{self, Tracer};
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tokio::runtime::Runtime;
use tracing::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry};
use tracing_subscriber::registry::LookupSpan;
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

    let layers = registry().with(layer).with(format);

    match runtime.block_on(tracer(&version.name))? {
        Some(tracer) => layers.with(tracer).init(),
        None         => layers.init(),
    };

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

async fn tracer<S>(service: &str) -> Result<Option<OpenTelemetryLayer<S, Tracer>>, Error>
where
    S: Subscriber + for <'s> LookupSpan<'s>,
{
    let endpoint = match var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(value)                 => value,
        Err(VarError::NotPresent) => return Ok(None),
        Err(e)                    => return Err(e.into()),
    };

    let headers = var("OTEL_EXPORTER_OTLP_HEADERS").unwrap_or_default();
    let headers = headers.split(',').flat_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        let name  = name.to_owned();
        let value = value.to_owned();
        Some((name, value))
    }).collect::<HashMap<_, _>>();

    let config = ExportConfig {
        endpoint: endpoint,
        protocol: Protocol::HttpBinary,
        ..Default::default()
    };

    let exporter = opentelemetry_otlp::new_exporter().http()
        .with_export_config(config)
        .with_headers(headers);

    let service  = KeyValue::new(SERVICE_NAME, service.to_owned());
    let resource = Resource::new([service]);
    let config   = trace::config().with_resource(resource);

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(config)
        .install_batch(Tokio)?;

    Ok(Some(tracing_opentelemetry::layer().with_tracer(tracer)))
}
