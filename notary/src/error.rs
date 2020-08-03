use std::fmt;
use reqwest::StatusCode;

#[derive(Debug)]
pub enum Error {
    Application(String),
    Status(StatusCode, String),
    Transport(String),
    InvalidSignature,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::Application(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Transport(err.to_string())
    }
}

impl From<ed25519_compact::Error> for Error {
    fn from(err: ed25519_compact::Error) -> Self {
        match err {
            ed25519_compact::Error::InvalidSignature  => Error::InvalidSignature,
            ed25519_compact::Error::SignatureMismatch => Error::InvalidSignature,
            _                                         => Error::Application(err.to_string()),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Transport(err.to_string())
    }
}
