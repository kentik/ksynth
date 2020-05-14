use clap::{App, ArgMatches};
use yaml_rust::Yaml;

pub struct Args {
    version: String,
    detail:  String,
}

impl Args {
    pub fn new() -> Self {
        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("0.0.0");
        let version = option_env!("GIT_VERSION").unwrap_or(version);
        let commit  = option_env!("GIT_COMMIT").unwrap_or("<unknown>");
        let detail  = format!("{} ({})", version, commit);

        Self {
            version: version.to_string(),
            detail:  detail,
        }
    }

    pub fn version(&self) -> String {
        self.version.clone()
    }

    pub fn matches<'a>(&self, yaml: &'a Yaml) -> ArgMatches<'a> {
        let version = self.version.as_str();
        let detail  = self.detail.as_str();

        let app = App::from_yaml(yaml);
        let app = app.version(version);
        let app = app.long_version(detail);

        app.get_matches()
    }
}
