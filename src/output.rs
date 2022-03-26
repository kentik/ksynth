use std::collections::HashMap;
use std::str::FromStr;
use anyhow::{anyhow, Error, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum Output {
    Influx(Args),
    NewRelic(Args),
    Kentik,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Args {
    args: HashMap<String, String>,
}

impl Args {
    pub fn get(&self, name: &str) -> Result<&str> {
        match self.args.get(name) {
            Some(value) => Ok(value.as_str()),
            None        => Err(anyhow!("missing arg '{}'", name)),
        }
    }

    pub fn opt(&self, name: &str) -> Option<&str> {
        self.args.get(name).map(String::as_str)
    }
}

impl FromStr for Output {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.splitn(2, ',');
        let sink = split.next().unwrap_or("kentik");
        let rest = split.next().unwrap_or("");

        let args = rest.split(',').flat_map(|str| {
            let (k, v) = str.split_once('=')?;
            Some((k.to_owned(), v.to_owned()))
        }).collect::<HashMap<_, _>>();

        let args = Args { args };

        Ok(match sink {
            "influx"   => Output::Influx(args),
            "newrelic" => Output::NewRelic(args),
            "kentik"   => Output::Kentik,
            _          => return Err(anyhow!("{}", s)),
        })
    }
}
