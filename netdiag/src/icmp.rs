use std::convert::{TryFrom, TryInto};
// FIXME: proper error impl?
use anyhow::{anyhow, Error};

pub const HEADER_SIZE: usize = 8;

pub const ECHO_REPLY4:   u8 = 0;
pub const UNREACHABLE:   u8 = 3;
pub const ECHO_REQUEST4: u8 = 8;
pub const TIME_EXCEEDED: u8 = 11;

pub const ECHO_REQUEST6: u8 = 128;
pub const ECHO_REPLY6:   u8 = 129;

#[derive(Debug)]
pub enum IcmpV4Packet<'a> {
    EchoRequest(Echo<'a>),
    EchoReply(Echo<'a>),
    Unreachable(Unreachable<'a>),
    TimeExceeded(&'a [u8]),
    Other(u8, u8, &'a [u8]),
}

#[derive(Debug)]
pub enum IcmpV6Packet<'a> {
    EchoRequest(Echo<'a>),
    EchoReply(Echo<'a>),
    Other(u8, u8, &'a [u8]),
}

#[derive(Debug)]
pub struct Echo<'a> {
    pub id:   u16,
    pub seq:  u16,
    pub data: &'a [u8]
}

#[derive(Debug)]
pub enum Unreachable<'a> {
    Net(&'a [u8]),
    Host(&'a [u8]),
    Protocol(&'a [u8]),
    Port(&'a [u8]),
    Other(u8, &'a [u8]),
}

impl<'a> TryFrom<&'a [u8]> for IcmpV4Packet<'a> {
    type Error = Error;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        if slice.len() < HEADER_SIZE {
            return Err(anyhow!("invalid slice"));
        }

        let kind = u8::try_from(slice[0])?;
        let code = u8::try_from(slice[1])?;
        let rest = &slice[4..];

        Ok(match (kind, code) {
            (ECHO_REPLY4,    0) => IcmpV4Packet::EchoReply(rest.try_into()?),
            (UNREACHABLE,   _)  => IcmpV4Packet::Unreachable((code, rest).try_into()?),
            (ECHO_REQUEST4,  0) => IcmpV4Packet::EchoRequest(rest.try_into()?),
            (TIME_EXCEEDED, _)  => IcmpV4Packet::TimeExceeded(&rest[4..]),
            _                   => IcmpV4Packet::Other(kind, code, rest),
        })
    }
}

impl<'a> TryFrom<&'a [u8]> for IcmpV6Packet<'a> {
    type Error = Error;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        if slice.len() < HEADER_SIZE {
            return Err(anyhow!("invalid slice"));
        }

        let kind = u8::try_from(slice[0])?;
        let code = u8::try_from(slice[1])?;
        let rest = &slice[4..];

        Ok(match (kind, code) {
            (ECHO_REQUEST6,  0) => IcmpV6Packet::EchoRequest(rest.try_into()?),
            (ECHO_REPLY6,    0) => IcmpV6Packet::EchoReply(rest.try_into()?),
            _                   => IcmpV6Packet::Other(kind, code, rest),
        })
    }
}

impl<'a> TryFrom<&'a [u8]> for Echo<'a> {
    type Error = Error;

    fn try_from(slice: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(Self {
            id:   u16::from_be_bytes(slice[0..2].try_into()?),
            seq:  u16::from_be_bytes(slice[2..4].try_into()?),
            data: &slice[4..]
        })
    }
}

impl<'a> TryFrom<(u8, &'a [u8])> for Unreachable<'a> {
    type Error = Error;

    fn try_from((code, slice): (u8, &'a [u8])) -> Result<Self, Self::Error> {
        let data = &slice[4..];
        Ok(match code {
            0 => Unreachable::Net(data),
            1 => Unreachable::Host(data),
            2 => Unreachable::Protocol(data),
            3 => Unreachable::Port(data),
            c => Unreachable::Other(c, data),
        })
    }
}

pub fn ping4<'a>(buf: &'a mut [u8], id: u16, seq: u16, payload: &[u8]) -> Result<&'a [u8], Error> {
    let n = HEADER_SIZE + payload.len();

    if buf.len() < n {
        return Err(anyhow!("invalid slice"))
    }

    buf[0..2].copy_from_slice(&[ECHO_REQUEST4, 0]);
    buf[2..4].copy_from_slice(&0u16.to_be_bytes());
    buf[4..6].copy_from_slice(&id.to_be_bytes());
    buf[6..8].copy_from_slice(&seq.to_be_bytes());
    buf[8..n].copy_from_slice(payload);

    let cksum = checksum(buf).to_be_bytes();
    buf[2..4].copy_from_slice(&cksum);

    Ok(&buf[0..n])
}

pub fn ping6<'a>(buf: &'a mut [u8], id: u16, seq: u16, payload: &[u8]) -> Result<&'a [u8], Error> {
    let n = HEADER_SIZE + payload.len();

    if buf.len() < n {
        return Err(anyhow!("invalid slice"))
    }

    buf[0..2].copy_from_slice(&[ECHO_REQUEST6, 0]);
    buf[2..4].copy_from_slice(&0u16.to_be_bytes());
    buf[4..6].copy_from_slice(&id.to_be_bytes());
    buf[6..8].copy_from_slice(&seq.to_be_bytes());
    buf[8..n].copy_from_slice(payload);

    Ok(&buf[0..n])
}

pub fn checksum(pkt: &[u8]) -> u16 {
    let mut sum = 0u32;

    for chunk in pkt.chunks(2) {
        let word = match chunk {
            [x, y] => u16::from_be_bytes([*x, *y]),
            [x]    => u16::from_be_bytes([*x, 0]),
            _      => unreachable!(),
        } as u32;
        sum = sum.wrapping_add(word);
    }

    while (sum >> 16) > 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    !sum as u16
}
