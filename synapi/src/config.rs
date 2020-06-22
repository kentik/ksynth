#[derive(Debug)]
pub struct Config {
    pub name:    String,
    pub global:  bool,
    pub region:  String,
    pub version: String,
    pub company: Option<u64>,
    pub proxy:   Option<String>,
    pub port:    u32,
}
