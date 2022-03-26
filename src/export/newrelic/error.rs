use std::fmt;
use hyper::StatusCode;

#[derive(Debug)]
pub enum Error {
    Authentication,
    Transport(String),
    Status(StatusCode),
    Other(String),
}

impl From<std::time::SystemTimeError> for Error {
    fn from(err: std::time::SystemTimeError) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<http::Error> for Error {
    fn from(err: http::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<hyper::header::InvalidHeaderValue> for Error {
    fn from(err: hyper::header::InvalidHeaderValue) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Transport(err.to_string())
    }
}

impl From<http::uri::InvalidUri> for Error {
    fn from(err: http::uri::InvalidUri) -> Self {
        Self::Transport(err.to_string())
    }
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Authentication => write!(f, "authentication failure"),
            Error::Transport(err) => write!(f, "transport error: {}", err),
            Error::Status(status) => write!(f, "status code {}", status),
            Error::Other(error)   => write!(f, "general error {}", error),
        }
    }
}
