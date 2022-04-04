use anyhow::Result;
use tracing::Subscriber;
use tracing_subscriber::reload::{Handle, Layer};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

pub struct Filter<S> {
    handle: Handle<EnvFilter, S>,
    root:   &'static str,
    level:  u64
}

impl<S: Subscriber> Filter<S> {
    pub fn new(root: &'static str, level: u64) -> Result<(Self, Layer<EnvFilter, S>)> {
        let filter = EnvFilter::from_default_env();
        let (layer, handle) = Layer::new(filter);

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
        let build = |app, lib: LevelFilter| -> Result<EnvFilter> {
            let mut filter = EnvFilter::from_default_env();

            let app = format!("{}={}", self.root, app).parse()?;
            let lib = lib.into();

            filter = filter.add_directive(app);
            filter = filter.add_directive(lib);

            Ok(filter)
        };

        self.handle.reload(match self.level {
            0 => build(LevelFilter::INFO,  LevelFilter::WARN)?,
            1 => build(LevelFilter::DEBUG, LevelFilter::INFO)?,
            2 => build(LevelFilter::TRACE, LevelFilter::INFO)?,
            3 => build(LevelFilter::TRACE, LevelFilter::DEBUG)?,
            _ => build(LevelFilter::TRACE, LevelFilter::TRACE)?,
        })?;

        Ok(())
    }
}
