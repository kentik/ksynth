use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddrV4};
use anyhow::Result;
use etherparse::*;

#[derive(Debug)]
pub struct Probe {
    pub src: SocketAddrV4,
    pub dst: SocketAddrV4,
    pub ttl: u8,
}

impl Probe {
    pub fn new(src: SocketAddrV4, dst: SocketAddrV4, ttl: u8) -> Self {
        Self { src, dst, ttl }
    }

    pub fn decode(pkt: &[u8]) -> Result<Option<Self>> {
        let (pkt, tail) = Ipv4Header::read_from_slice(pkt)?;
        if let ip @ Ipv4Header { protocol: UDP, .. } = pkt {
            let src = Ipv4Addr::from(ip.source);
            let dst = Ipv4Addr::from(ip.destination);

            let pkt = UdpHeaderSlice::from_slice(&tail)?;
            let src = SocketAddrV4::new(src, pkt.source_port());
            let dst = SocketAddrV4::new(dst, pkt.destination_port());

            return Ok(Some(Probe { src, dst, ttl: 0 }))
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

const UDP: u8 = IpTrafficClass::Udp as u8;
