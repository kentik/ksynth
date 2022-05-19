use anyhow::Result;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry, reload};
use tracing_subscriber::filter::{Directive, EnvFilter, LevelFilter};
use super::{tracer, Handle};

pub struct Handles {
    pub filter: Box<dyn Handle<Directive> + Send + Sync>,
    pub print:  Box<dyn Handle<LevelFilter> + Send + Sync>,
    pub export: Option<Box<dyn Handle<LevelFilter> + Send + Sync>>,
}

pub async fn setup(root: &str, level: u64) -> Result<Handles> {
    let (app, lib) = match level {
        0 => (LevelFilter::INFO,  LevelFilter::WARN),
        1 => (LevelFilter::DEBUG, LevelFilter::INFO),
        2 => (LevelFilter::TRACE, LevelFilter::INFO),
        3 => (LevelFilter::TRACE, LevelFilter::DEBUG),
        _ => (LevelFilter::TRACE, LevelFilter::TRACE),
    };

    let app = format!("{root}={app}");

    let mut filter = EnvFilter::from_default_env();
    filter = filter.add_directive(app.parse()?);
    filter = filter.add_directive(lib.into());

    let print  = fmt::layer().compact().with_filter(LevelFilter::TRACE);

    let filter = reload::Layer::new(filter);
    let print  = reload::Layer::new(print);
    let export = tracer(root).await?.map(|tracer| {
        let layer = tracer.with_filter(LevelFilter::TRACE);
        let (layer, handle) = reload::Layer::new(layer);
        (Some(layer), Some(Box::new(handle) as Box<_>))
    }).unwrap_or_default();

    let layers = registry().with(filter.0).with(print.0);

    match export.0 {
        Some(layer) => layers.with(layer).init(),
        None        => layers.init()
    };

    Ok(Handles {
        filter: Box::new(filter.1),
        print:  Box::new(print.1),
        export: export.1,
    })
}

impl Handles {
    pub fn filter(&self, filter: Directive) -> Result<()> {
        self.filter.modify(filter)
    }

    pub fn print(&self, level: LevelFilter) -> Result<()> {
        self.print.modify(level)
    }

    pub fn export(&self, level: LevelFilter) -> Result<()> {
        match &self.export {
            Some(handle) => handle.modify(level),
            None         => Ok(()),
        }
    }
}
