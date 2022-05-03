use std::mem::take;
use anyhow::Result;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{Directive, EnvFilter, Filtered, LevelFilter};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload;

pub struct Filter {
    handle: Box<dyn Handle<Directive> + Send + Sync>,
    root:   &'static str,
    level:  u64
}

pub trait Handle<C> {
    fn modify(&self, c: C) -> Result<()>;
}

impl Filter {
    pub fn new<S: Subscriber>(root: &'static str, level: u64) -> Result<(Self, reload::Layer<EnvFilter, S>)>
    where
        S: Subscriber + for <'s> LookupSpan<'s>,
    {
        let filter = EnvFilter::from_default_env();
        let (layer, handle) = reload::Layer::new(filter);
        let handle = Box::new(handle);

        let mut filter = Self { handle, root, level };
        filter.apply()?;

        Ok((filter, layer))
    }

    pub fn increment(&mut self) -> Result<(u64, u64)> {
        let old = self.level;
        let new = old.wrapping_add(1) % 5;

        self.level = new;
        self.apply()?;

        Ok((old, new))
    }

    fn apply(&mut self) -> Result<()> {
        let set = |app, lib: LevelFilter| -> Result<()> {
            let app = format!("{}={}", self.root, app).parse()?;
            let lib = lib.into();

            self.handle.modify(app)?;
            self.handle.modify(lib)?;

            Ok(())
        };

        match self.level {
            0 => set(LevelFilter::INFO,  LevelFilter::WARN)?,
            1 => set(LevelFilter::DEBUG, LevelFilter::INFO)?,
            2 => set(LevelFilter::TRACE, LevelFilter::INFO)?,
            3 => set(LevelFilter::TRACE, LevelFilter::DEBUG)?,
            _ => set(LevelFilter::TRACE, LevelFilter::TRACE)?,
        };

        Ok(())
    }
}

impl<S> Handle<Directive> for reload::Handle<EnvFilter, S>
where
    S: Subscriber + for <'s> LookupSpan<'s>
{
    fn modify(&self, d: Directive) -> Result<()> {
        Ok(self.modify(|layer| {
            let mut filter = take(layer);
            filter = filter.add_directive(d);
            *layer = filter;
        })?)
    }
}

impl<S> Handle<LevelFilter> for reload::Handle<LevelFilter, S>
where
    S: Subscriber + for <'s> LookupSpan<'s>
{
    fn modify(&self, level: LevelFilter) -> Result<()> {
        Ok(self.modify(|layer| *layer = level)?)
    }

}

impl<L, S> Handle<Directive> for reload::Handle<Filtered<L, EnvFilter, S>, S>
where
    L: Layer<S>,
    S: Subscriber + for <'s> LookupSpan<'s>,
{
    fn modify(&self, d: Directive) -> Result<()> {
        Ok(self.modify(|layer| {
            let mut filter = take(layer.filter_mut());
            filter = filter.add_directive(d);
            *layer.filter_mut() = filter;
        })?)
    }
}

impl<L, S> Handle<LevelFilter> for reload::Handle<Filtered<L, LevelFilter, S>, S>
where
    L: Layer<S>,
    S: Subscriber + for <'s> LookupSpan<'s>,
{
    fn modify(&self, level: LevelFilter) -> Result<()> {
        Ok(self.modify(|layer| *layer.filter_mut() = level)?)
    }
}
