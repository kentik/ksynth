use anyhow::Result;
use serde_json::Value;
use v8::{new_default_platform, V8};
use v8vm::{self, Function, Guard, Handle};
use v8vm::ex::Fetch;
use crate::cfg::Config;
use crate::net::http::HttpClient;
use super::fetch::FetchClient;

pub struct Machine {
    handle: Handle,
    _guard: Guard,
}

pub struct Export {
    export: Function,
}

pub fn initialize() {
    let platform = new_default_platform(0, false).make_shared();
    V8::initialize_platform(platform);
    V8::initialize();
}

impl Machine {
    pub async fn new(cfg: &Config) -> Result<Self> {
        let Config { bind, resolver, roots, .. } = cfg.clone();

        let handle = tokio::runtime::Handle::current();
        let client = HttpClient::new(bind, resolver, roots)?;
        let fetch  = Fetch::new(FetchClient::new(client, handle));

        let mut machine = v8vm::Machine::new(CODE.to_owned());
        machine.extend(fetch);
        let (handle, _guard) = machine.exec();

        Ok(Self { handle, _guard })
    }

    pub fn find(&self, export: &str) -> Result<Export> {
        let export = self.handle.find(export)?;
        Ok(Export { export })
    }
}

impl Export {
    pub async fn call(&self, arg: Value) -> Result<Value> {
        Ok(self.export.call(arg)?.await?)
    }
}

const CODE: &str = include_str!("blob.js");
