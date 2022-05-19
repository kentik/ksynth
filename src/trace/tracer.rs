use std::collections::HashMap;
use std::env::{var, VarError};
use anyhow::Result;
use opentelemetry::KeyValue;
use opentelemetry::runtime::Tokio;
use opentelemetry::sdk::Resource;
use opentelemetry::sdk::trace::{self, Tracer};
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;

pub async fn tracer<S>(service: &str) -> Result<Option<OpenTelemetryLayer<S, Tracer>>>
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
