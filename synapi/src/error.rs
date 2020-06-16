use std::fmt;
use std::time::Duration;
use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Debug)]
pub enum Error {
    Application(Application),
    Backend(Backend),
    Session,
    Status(StatusCode),
    Transport(String),
    Unauthorized,
}

#[derive(Debug, Deserialize)]
pub struct Application {
    pub code:    u32,
    pub message: String,
    pub retry:   Option<Duration>,
}

#[derive(Debug, Deserialize)]
pub struct Backend {
    pub code:    i32,
    pub message: String,
}

#[derive(Debug)]
pub enum Retry {
    Delay(Duration),
    Default,
    None,
}

impl Error {
    pub fn retry(&self) -> Retry {
        match self {
            Error::Application(a) => a.retry(),
            Error::Backend(..)    => Retry::Default,
            Error::Session        => Retry::Default,
            Error::Status(..)     => Retry::Default,
            Error::Transport(..)  => Retry::Default,
            Error::Unauthorized   => Retry::None,
        }
    }
}

impl Application {
    pub fn retry(&self) -> Retry {
        match self.retry {
            Some(delay) => Retry::Delay(delay),
            None        => Retry::None,
        }
    }
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
