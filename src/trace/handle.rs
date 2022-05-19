use std::mem::take;
use anyhow::Result;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{Directive, EnvFilter, Filtered, LevelFilter};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload;

pub trait Handle<C> {
    fn modify(&self, new: C) -> Result<()>;
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
