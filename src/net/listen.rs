use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use anyhow::{anyhow, Error, Result};
use tracing::{debug, error};
use parking_lot::Mutex;
use tokio::net::{TcpListener, UdpSocket};
use tokio::task::JoinHandle;

#[derive(Debug, Eq, PartialEq)]
pub struct Addr {
    addr:  IpAddr,
    ports: Vec<Port>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Port {
    TCP(u16),
    UDP(u16),
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Addrs(Vec<Addr>);

#[derive(Clone)]
pub struct Listener {
    active: Arc<Mutex<HashMap<Port, JoinHandle<()>>>>,
    listen: Vec<IpAddr>,
}

impl Listener {
    pub async fn new(addrs: Addrs) -> Self {
        let mut active = HashMap::new();
        let mut listen = Vec::new();

        for Addr { addr, ports } in addrs.0 {
            listen.push(addr);

            for port in ports {
                let task = spawn(addr, port);
                active.insert(port, task);
            }
        }

        let active = Arc::new(Mutex::new(active));

        Self { active, listen }
    }

    pub async fn add(&self, port: Port) {
        let mut active = self.active.lock();
        for addr in &self.listen {
            active.entry(port).or_insert_with(|| spawn(*addr, port));
        }
    }
}

fn spawn(addr: IpAddr, port: Port) -> JoinHandle<()> {
    tokio::task::spawn(async move {
        let result = match port {
            Port::TCP(port) => listen(addr, port).await,
            Port::UDP(port) => accept(addr, port).await,
        };

        match result {
            Ok(()) => debug!("task finished"),
            Err(e) => error!("task failed: {e}"),
        };
    })
}

async fn listen(addr: IpAddr, port: u16) -> Result<()> {
    let addr = SocketAddr::new(addr, port);
    let sock = TcpListener::bind(&addr).await?;

    debug!("listening on {addr:?}");

    loop {
        sock.accept().await?;
    }
}

async fn accept(addr: IpAddr, port: u16) -> Result<()> {
    let addr = SocketAddr::new(addr, port);
    let sock = UdpSocket::bind(&addr).await?;

    debug!("accepting on {addr:?}");

    loop {
        sock.recv_from(&mut [0; 64]).await?;
    }
}

impl FromStr for Addrs {
    type Err = Error;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        let mut split = spec.split(',');

        let addr = match split.next() {
            Some(addr) => addr.parse()?,
            None       => return Err(anyhow!("invalid listen spec: {spec}")),
        };

        let ports = split.map(|port| {
            port.parse()
        }).collect::<Result<_>>()?;

        Ok(Self(vec![Addr { addr, ports }]))
    }
}

impl FromStr for Port {
    type Err = Error;

    fn from_str(spec: &str) -> Result<Self, Self::Err> {
        match spec.split_once('/') {
            Some((port, "tcp")) => Ok(Port::TCP(port.parse()?)),
            Some((port, "udp")) => Ok(Port::UDP(port.parse()?)),
            Some((_,    proto)) => Err(anyhow!("invalid protocol: {proto}")),
            None                => Err(anyhow!("invalid port spec: {spec}")),
        }
     }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use anyhow::Result;
    use super::{Addr, Addrs, Port};
    use Port::*;

    #[test]
    fn parse() -> Result<()> {
        assert_eq!(Addrs(vec![
            Addr {
                addr:  "0.0.0.0".parse()?,
                ports: vec![],
            }
        ]), "0.0.0.0".parse()?);

        assert_eq!(Addrs(vec![
            Addr {
                addr:  "0.0.0.0".parse()?,
                ports: vec![TCP(80)],
            }
        ]), "0.0.0.0,80/tcp".parse()?);

        assert_eq!(Addrs(vec![
            Addr {
                addr:  "127.0.0.1".parse()?,
                ports: vec![TCP(80), UDP(81)],
            }
        ]), "127.0.0.1,80/tcp,81/udp".parse()?);

        Ok(())
    }

    #[test]
    fn invalid() -> Result<()> {
        assert!(Addrs::from_str("abcd").is_err());
        assert!(Addrs::from_str("0.0.0.0,80").is_err());

        assert!(Port::from_str("abcd").is_err());
        assert!(Port::from_str("1234").is_err());
        assert!(Port::from_str("80/sctp").is_err());

        Ok(())
    }
}
