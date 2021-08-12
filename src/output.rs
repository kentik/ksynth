use std::collections::HashMap;
use std::str::FromStr;
use anyhow::{anyhow, Error, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum Output {
    Influx(String, Args),
    Kentik,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Args {
    args: HashMap<String, String>,
}

impl Args {
    pub fn get(&self, key: &str) -> Option<&String> {
        self.args.get(key)
    }
}

impl FromStr for Output {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.splitn(2, '=');
        let format = split.next().unwrap_or("kentik");
        let rest   = split.next().unwrap_or("");

        let mut split = rest.split(',').filter(|s| !s.is_empty());
        let arg  = split.next().map(str::to_owned);
        let args = split.flat_map(|str| {
            let (k, v) = str.split_once('=')?;
            Some((k.to_owned(), v.to_owned()))
        }).collect::<HashMap<_, _>>();
        let args = Args { args };

        match (format, arg) {
            ("influx", Some(url)) => Ok(Output::Influx(url, args)),
            ("kentik", None)      => Ok(Output::Kentik),
            _                     => Err(anyhow!("{}", s)),
        }
    }
}
