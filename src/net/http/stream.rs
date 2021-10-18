use std::convert::TryFrom;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use anyhow::{Error, Result};
use netdiag::Bind;
use rustls::Session;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpSocket, TcpStream};
use tokio_rustls::client::TlsStream;
use crate::net::tls::Identity;

pub struct Connection {
    peer:   Peer,
    stream: Stream,
}

#[derive(Clone, Debug)]
pub struct Peer {
    pub addr:   SocketAddr,
    pub server: Identity,
}

enum Stream {
    TCP(TcpStream),
    TLS(TlsStream<TcpStream>),
}

impl Connection {
    pub fn http2(&self) -> bool {
        (match &self.stream {
            Stream::TCP(_) => None,
            Stream::TLS(s) => s.get_ref().1.get_alpn_protocol(),
        } == Some(b"h2"))
    }

    pub fn peer(&self) -> Peer {
        self.peer.clone()
    }
}

pub async fn socket(bind: &Bind, addr: &SocketAddr) -> Result<TcpSocket> {
    let (socket, bind) = match addr {
        SocketAddr::V4(_) => (TcpSocket::new_v4()?, bind.sa4()),
        SocketAddr::V6(_) => (TcpSocket::new_v6()?, bind.sa6()),
    };

    socket.bind(bind)?;

    Ok(socket)
}

impl TryFrom<(TcpStream, Identity)> for Connection {
    type Error = Error;

    fn try_from((tcp, server): (TcpStream, Identity)) -> Result<Self, Self::Error> {
        let peer = Peer {
            addr:   tcp.peer_addr()?,
            server: server,
        };
        let stream = Stream::TCP(tcp);
        Ok(Connection { peer, stream })
    }
}

impl TryFrom<(TlsStream<TcpStream>, Identity)> for Connection {
    type Error = Error;

    fn try_from((tls, server): (TlsStream<TcpStream>, Identity)) -> Result<Self, Self::Error> {
        let peer = Peer {
            addr:   tls.get_ref().0.peer_addr()?,
            server: server,
        };
        let stream = Stream::TLS(tls);
        Ok(Connection { peer, stream })
    }
}

impl AsyncRead for Connection {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut ReadBuf<'_>) -> Poll<Result<(), io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_read(cx, buf),
            Stream::TLS(tls) => Pin::new(tls).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Connection {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_write(cx, buf),
            Stream::TLS(tls) => Pin::new(tls).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_flush(cx),
            Stream::TLS(tls) => Pin::new(tls).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
        match &mut Pin::get_mut(self).stream {
            Stream::TCP(tcp) => Pin::new(tcp).poll_shutdown(cx),
            Stream::TLS(tls) => Pin::new(tls).poll_shutdown(cx),
        }
    }
}
