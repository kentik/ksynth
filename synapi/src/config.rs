use std::net::IpAddr;
use std::str::FromStr;
use rustls::RootCertStore;

#[derive(Debug)]
pub struct Config {
    pub name:    String,
    pub global:  bool,
    pub region:  Region,
    pub version: String,
    pub machine: String,
    pub company: Option<u64>,
    pub site:    Option<u64>,
    pub proxy:   Option<String>,
    pub bind:    Option<IpAddr>,
    pub roots:   RootCertStore,
}

#[derive(Clone, Debug)]
pub struct Region  {
    pub name: String,
    pub api:  String,
    pub flow: String,
}

impl Region  {
    fn named(name: &str) -> Self {
        let name   = name.to_ascii_uppercase();
        let domain = match name.as_str() {
            "US" => "kentik.com".to_owned(),
            "EU" => "kentik.eu".to_owned(),
            name => format!("{}.kentik.com", name.to_ascii_lowercase()),
        };

        Self {
            name: name,
            api:  format!("https://api.{}/api/agent/v1/syn", domain),
            flow: format!("https://flow.{}/chf",             domain),
        }
    }

    fn custom(name: &str, api: &str, flow: &str) -> Self {
        Self {
            name: name.to_owned(),
            api:  api.to_owned(),
            flow: flow.to_owned(),
        }
    }
}

impl Default for Region {
    fn default() -> Self {
        Self::named("US")
    }
}

impl FromStr for Region {
    type Err = String;

    fn from_str(region: &str) -> Result<Self, Self::Err> {
        match &region.split(',').collect::<Vec<_>>()[..] {
            [name]            => Ok(Region::named(name)),
            [name, api, flow] => Ok(Region::custom(name, api, flow)),
            _                 => Err("invalid region".to_owned()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn region_us() {
        let region = "us".parse::<Region>().unwrap();
        assert_eq!(&region.name, "US");
        assert_eq!(&region.api,  "https://api.kentik.com/api/agent/v1/syn");
        assert_eq!(&region.flow, "https://flow.kentik.com/chf");
    }

    #[test]
    fn region_eu() {
        let region = "EU".parse::<Region>().unwrap();
        assert_eq!(&region.name, "EU");
        assert_eq!(&region.api,  "https://api.kentik.eu/api/agent/v1/syn");
        assert_eq!(&region.flow, "https://flow.kentik.eu/chf");
    }

    #[test]
    fn region_jp1() {
        let region = "jp1".parse::<Region>().unwrap();
        assert_eq!(&region.name, "JP1");
        assert_eq!(&region.api,  "https://api.jp1.kentik.com/api/agent/v1/syn");
        assert_eq!(&region.flow, "https://flow.jp1.kentik.com/chf");
    }

    #[test]
    fn region_custom() {
        let region = "test,http://foo,http://bar:1234/baz".parse::<Region>().unwrap();
        assert_eq!(&region.name, "test");
        assert_eq!(&region.api,  "http://foo");
        assert_eq!(&region.flow, "http://bar:1234/baz");
    }
}
