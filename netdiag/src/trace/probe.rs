use std::io::Cursor;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use anyhow::{anyhow, Result};
use etherparse::*;

#[derive(Debug)]
pub enum Probe {
    V4(ProbeV4),
    V6(ProbeV6),
}

#[derive(Debug)]
pub struct ProbeV4 {
    pub src: SocketAddrV4,
    pub dst: SocketAddrV4,
    pub ttl: u8,
}

#[derive(Debug)]
pub struct ProbeV6 {
    pub src: SocketAddrV6,
    pub dst: SocketAddrV6,
    pub ttl: u8,
}

impl Probe {
    pub fn new(src: SocketAddr, dst: SocketAddr, ttl: u8) -> Result<Self> {
        let probe4  = |src, dst| Probe::V4(ProbeV4 { src, dst, ttl });
        let probe6  = |src, dst| Probe::V6(ProbeV6 { src, dst, ttl });
        let invalid = || anyhow!("mixed IPv4 and IPv6 addresses");

        match (src, dst) {
            (SocketAddr::V4(src), SocketAddr::V4(dst)) => Ok(probe4(src, dst)),
            (SocketAddr::V6(src), SocketAddr::V6(dst)) => Ok(probe6(src, dst)),
            _                                          => Err(invalid()),
        }
    }

    pub fn src(&self) -> SocketAddr {
        match self {
            Self::V4(v4) => SocketAddr::V4(v4.src),
            Self::V6(v6) => SocketAddr::V6(v6.src),
        }
    }

    pub fn dst(&self) -> SocketAddr {
        match self {
            Self::V4(v4) => SocketAddr::V4(v4.dst),
            Self::V6(v6) => SocketAddr::V6(v6.dst),
        }
    }

    pub fn ttl(&self) -> u8 {
        match self {
            Self::V4(v4) => v4.ttl,
            Self::V6(v6) => v6.ttl,
        }
    }
}

impl ProbeV4 {
    pub fn decode(pkt: &[u8]) -> Result<Option<Probe>> {
        let (pkt, tail) = Ipv4Header::read_from_slice(pkt)?;
        if let ip @ Ipv4Header { protocol: UDP, .. } = pkt {
            let src = Ipv4Addr::from(ip.source);
            let dst = Ipv4Addr::from(ip.destination);

            let pkt = UdpHeaderSlice::from_slice(&tail)?;
            let src = SocketAddrV4::new(src, pkt.source_port());
            let dst = SocketAddrV4::new(dst, pkt.destination_port());

            let probe = Probe::V4(ProbeV4 { src, dst, ttl: 0 });

            return Ok(Some(probe))
        }
        Ok(None)
    }

    pub fn encode<'a>(&self, buf: &'a mut [u8]) -> Result<&'a [u8]> {
        let mut buf = Cursor::new(buf);

        let src = self.src.ip().octets();
        let dst = self.dst.ip().octets();

        let pkt = PacketBuilder::ipv4(src, dst, self.ttl);
        let pkt = pkt.udp(self.src.port(), self.dst.port());

        let n = pkt.size(0);
        pkt.write(&mut buf, &[])?;

        Ok(&buf.into_inner()[..n])
    }
}

impl ProbeV6 {
    pub fn decode(pkt: &[u8]) -> Result<Option<Probe>> {
        let (pkt, tail) = Ipv6Header::read_from_slice(&pkt)?;
        if let ip @ Ipv6Header { next_header: UDP, .. } = pkt {
            let src = Ipv6Addr::from(ip.source);
            let dst = Ipv6Addr::from(ip.destination);

            let pkt = UdpHeaderSlice::from_slice(&tail)?;
            let src = SocketAddrV6::new(src, pkt.source_port(), 0, 0);
            let dst = SocketAddrV6::new(dst, pkt.destination_port(), 0, 0);

            let probe = Probe::V6(ProbeV6 { src, dst, ttl: 0 });

            return Ok(Some(probe))
        }
        Ok(None)
    }

    pub fn encode<'a>(&self, buf: &'a mut [u8]) -> Result<&'a [u8]> {
        let mut buf = Cursor::new(buf);

        let src = self.src.port();
        let dst = self.dst.port();
        let pkt = UdpHeader::without_ipv4_checksum(src, dst, 0)?;

        pkt.write(&mut buf)?;
        let n = buf.position() as usize;

        Ok(&buf.into_inner()[..n])
    }

}

const UDP: u8 = IpTrafficClass::Udp as u8;
