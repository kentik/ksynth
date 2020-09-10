use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;
use clap::{ArgMatches, Error, ErrorKind};
use yaml_rust::Yaml;

#[derive(Debug)]
pub struct Args<'a, 'y> {
    args: &'a ArgMatches<'y>,
    yaml: &'y Yaml,
    vars: Rc<HashMap<String, String>>,
}

impl<'a, 'y> Args<'a, 'y> {
    pub fn new(args: &'a ArgMatches<'y>, yaml: &'y Yaml) -> Self {
        let vars = Self::vars(yaml).unwrap_or_default();
        Self { args, yaml, vars }
    }

    pub fn is_present(&self, name: &str) -> bool {
        self.args.is_present(name) || self.is_set(name)
    }

    pub fn subcommand(&self) -> Option<(&str, Args<'a, 'y>)> {
        match self.args.subcommand() {
            (name, Some(args)) => self.subargs(name, args),
            _                  => None,
        }
    }

    pub fn opt<T: FromStr>(&self, name: &str) -> Result<Option<T>, Error> where T::Err: Display {
        self.value_of(name).map(T::from_str).transpose().map_err(|e| {
            let msg = format!("invalid value for {}: {}", name, e);
            Error::with_description(&msg, ErrorKind::InvalidValue)
        })
    }

    fn subargs<'n>(&self, name: &'n str, args: &'a ArgMatches<'y>) -> Option<(&'n str, Self)> {
        let cmds = self.yaml["subcommands"].as_vec()?;
        let yaml = cmds.iter().flat_map(|yaml| {
            match &yaml[name] {
                yaml @ Yaml::Hash(_) => Some(yaml),
                _                    => None,
            }
        }).next()?;
        let vars = Rc::clone(&self.vars);
        Some((name, Self { args, yaml, vars }))
    }

    fn is_set(&self, name: &str) -> bool {
        self.vars.get(name).map(|value| {
            value == "" || value.eq_ignore_ascii_case("true")
        }).unwrap_or(false)
    }

    fn vars(yaml: &Yaml) -> Option<Rc<HashMap<String, String>>> {
        let mut vars = HashMap::new();

        for arg in yaml["args"].as_vec()? {
            let (name, args) = arg.as_hash()?.into_iter().next()?;
            if let Some(var) = args["env"].as_str() {
                if let Some(value) = env::var_os(var) {
                    let name  = name.as_str()?.to_owned();
                    let value = value.to_string_lossy().into_owned();
                    vars.insert(name, value);
                }
            }
        }

        for cmd in yaml["subcommands"].as_vec()? {
            let (_, cmd) = cmd.as_hash()?.into_iter().next()?;
            for arg in cmd["args"].as_vec()? {
                let (name, args) = arg.as_hash()?.into_iter().next()?;
                if let Some(var) = args["env"].as_str() {
                    if let Some(value) = env::var_os(var) {
                        let name  = name.as_str()?.to_owned();
                        let value = value.to_string_lossy().into_owned();
                        vars.insert(name, value);
                    }
                }
            }
        }

        Some(Rc::new(vars))
    }
}

impl<'a, 'y> Deref for Args<'a, 'y> {
    type Target = ArgMatches<'y>;

    fn deref(&self) -> &Self::Target {
        &self.args
    }
}
