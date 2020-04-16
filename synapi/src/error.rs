use std::fmt;
use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Debug)]
pub enum Error {
    Application(u32, String),
    Backend(Backend),
    Status(StatusCode),
    Transport(String),
    Unauthorized,
}

#[derive(Debug, Deserialize)]
pub struct Backend {
    code:    i32,
    message: String,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<Backend> for Error {
    fn from(err: Backend) -> Self {
        Error::Backend(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Transport(err.to_string())
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Transport(err.to_string())
    }
}

impl From<StatusCode> for Error {
    fn from(err: StatusCode) -> Self {
        Error::Status(err)
    }
}
