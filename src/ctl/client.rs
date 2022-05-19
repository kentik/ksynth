use std::path::Path;
use std::io::{Error, ErrorKind};
use anyhow::Result;
use futures::prelude::*;
use tokio::net::UnixStream;
use tokio_util::codec::{Decoder, Framed};
use tokio_util::codec::length_delimited::LengthDelimitedCodec;
use serde::de::DeserializeOwned;
use super::Command;
use ErrorKind::UnexpectedEof;

pub struct Client {
    codec: Framed<UnixStream, LengthDelimitedCodec>,
}

impl Client {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let stream = UnixStream::connect(path).await?;
        let codec  = LengthDelimitedCodec::new().framed(stream);
        Ok(Self { codec })
    }

    pub async fn send<T: DeserializeOwned>(&mut self, cmd: Command) -> Result<T> {
        let cmd = serde_json::to_vec(&cmd)?;

        self.codec.send(cmd.into()).await?;

        match self.codec.next().await {
            Some(frame) => Ok(serde_json::from_slice(&frame?)?),
            None        => Err(Error::from(UnexpectedEof).into()),
        }
    }
}
