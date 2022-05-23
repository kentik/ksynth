use std::fs::remove_file;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use futures::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use tracing::{debug, error};
use tracing_subscriber::filter::{Directive, LevelFilter, ParseError};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{channel, Sender};
use tokio::task::spawn;
use tokio_util::codec::Decoder;
use tokio_util::codec::length_delimited::LengthDelimitedCodec;
use crate::status::Report;
use crate::trace::Handles;
use crate::watch::Event;

pub struct Server {
    sock:  PathBuf,
    state: Arc<State>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Command {
    Status,
    Trace(Trace),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Response {
    Empty,
    Report(Report),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Trace {
    Filter(Filter),
    Print(Level),
    Export(Level),
}

#[derive(Debug)]
pub struct Filter(Directive);

#[derive(Debug)]
pub struct Level(LevelFilter);

struct State {
    handles: Handles,
    report:  Sender<Event>,
}

impl Server {
    pub fn new(sock: PathBuf, handles: Handles, report: Sender<Event>) -> Self {
        let state = Arc::new(State { handles, report  });
        Self { sock, state }
    }

    pub async fn exec(self) -> Result<()> {
        let sock = UnixListener::bind(&self.sock)?;

        loop {
            let (stream, _addr) = sock.accept().await?;
            let state = self.state.clone();
            spawn(async move {
                match handle(stream, state).await {
                    Ok(()) => debug!("stream finished"),
                    Err(e) => error!("stream error: {e:?}"),
                }
            });
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = remove_file(&self.sock);
    }
}

async fn handle(stream: UnixStream, state: Arc<State>) -> Result<()> {
    let mut codec = LengthDelimitedCodec::new().framed(stream);

    while let Some(frame) = codec.next().await {
        let response = match serde_json::from_slice(&frame?)? {
            Command::Status   => status(&state.report).await?,
            Command::Trace(r) => trace(r, &state.handles)?,
        };
        let response = serde_json::to_vec(&response)?;
        codec.send(response.into()).await?;
    }

    Ok(())
}

async fn status(events: &Sender<Event>) -> Result<Response> {
    let (tx, mut rx) = channel(1);

    let request = Event::Report(tx);
    events.send(request).await?;

    match rx.recv().await {
        Some(report) => Ok(Response::Report(report)),
        None         => Err(anyhow!("report missing")),
    }
}

fn trace(trace: Trace, handles: &Handles) -> Result<Response> {
    match trace {
        Trace::Filter(filter) => handles.filter(filter.0)?,
        Trace::Print(level)   => handles.print(level.0)?,
        Trace::Export(level)  => handles.export(level.0)?,
    };
    Ok(Response::Empty)
}

impl FromStr for Filter {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl FromStr for Level {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

impl<'d> Deserialize<'d> for Filter {
    fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let str = String::deserialize(deserializer)?;
        Ok(Filter(str.parse().map_err(Error::custom)?))
    }
}

impl Serialize for Filter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'d> Deserialize<'d> for Level {
    fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let str = String::deserialize(deserializer)?;
        Ok(Level(str.parse().map_err(Error::custom)?))
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
