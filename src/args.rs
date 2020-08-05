use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::ops::Deref;
use std::rc::Rc;
use clap::ArgMatches;
use yaml_rust::Yaml;

#[derive(Debug)]
pub struct Args<'a, 'y> {
    args: &'a ArgMatches<'y>,
    yaml: &'y Yaml,
    vars: Rc<HashMap<String, OsString>>,
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
        self.vars.contains_key(name)
    }

    fn vars(yaml: &Yaml) -> Option<Rc<HashMap<String, OsString>>> {
        let mut vars = HashMap::new();

        for arg in yaml["args"].as_vec()? {
            let (name, args) = arg.as_hash()?.into_iter().next()?;
            if let Some(var) = args["env"].as_str() {
                if let Some(value) = env::var_os(var) {
                    let name  = name.as_str()?;
                    vars.insert(name.to_owned(), value);
                }
            }
        }

        for cmd in yaml["subcommands"].as_vec()? {
            let (_, cmd) = cmd.as_hash()?.into_iter().next()?;
            for arg in cmd["args"].as_vec()? {
                let (name, args) = arg.as_hash()?.into_iter().next()?;
                if let Some(var) = args["env"].as_str() {
                    if let Some(value) = env::var_os(var) {
                        let name  = name.as_str()?;
                        vars.insert(name.to_owned(), value);
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
