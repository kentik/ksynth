use std::collections::HashMap;
use std::str::FromStr;
use anyhow::{anyhow, Error, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum Output {
    Influx(String, Auth),
    Kentik,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Auth {
    Basic(String, String),
    Token(String),
    None,
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
            str.split_once('=')
        }).collect::<HashMap<_, _>>();

        let token    = args.get("token");
        let username = args.get("username");
        let password = args.get("password");
        let basic    = username.zip(password);

        let auth = if let Some(token) = token {
            Auth::Token(token.to_string())
        } else if let Some((username, password)) = basic {
            Auth::Basic(username.to_string(), password.to_string())
        } else {
            Auth::None
        };

        match (format, arg) {
            ("influx", Some(url)) => Ok(Output::Influx(url, auth)),
            ("kentik", None)      => Ok(Output::Kentik),
            _                     => Err(anyhow!("{}", s)),
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use super::*;

    #[test]
    fn influx() -> Result<()> {
        let url      = "http://127.0.0.1:8080/api/v2/write";
        let username = "test@example.com";
        let password = "secret";
        let token    = "abcdef0123456789";

        assert_eq!(Output::Influx(url.to_owned(), Auth::None,
        ), parse(&[
            "influx", url,
        ])?);

        assert_eq!(Output::Influx(url.to_owned(), Auth::Basic(
            username.to_owned(),
            password.to_owned(),
        )), parse(&[
            "influx",   url,
            "username", username,
            "password", password,
        ])?);

        assert_eq!(Output::Influx(url.to_owned(), Auth::Token(
            token.to_owned(),
        )), parse(&[
            "influx", url,
            "token",  token,
        ])?);

        Ok(())
    }

    fn parse(parts: &[&str]) -> Result<Output> {
        Output::from_str(&parts.chunks(2).map(|chunk| {
            let name  = chunk[0];
            let value = chunk[1];
            format!("{}={}", name, value)
        }).collect::<Vec<_>>().join(","))
    }
}
