#![allow(unused)]

use std::convert::TryFrom;
use std::io::Error;
use std::os::raw::c_int;
use anyhow::Result;
use libc::*;
use super::{getuid, setuser};

pub fn apply(user: Option<&str>) -> Result<()> {
    if user.is_some() && getuid()? == 0 {
        set_securebits(SECBIT_KEEP_CAPS)?;
    }

    user.map(setuser).transpose()?;

    let mut caps = Caps::default();
    caps.effective |= CAP_NET_RAW;
    caps.permitted |= CAP_NET_RAW;
    Ok(capset(None, &caps)?)
}

#[derive(Clone, Debug, Default)]
struct Caps {
    effective:   u64,
    permitted:   u64,
    inheritable: u64,
}

impl Caps {
    fn new(data: &[Data; 2]) -> Self {
        let mut caps = Self::default();
        join(&mut caps.effective,   data[0].effective,   data[1].effective);
        join(&mut caps.permitted,   data[0].permitted,   data[1].permitted);
        join(&mut caps.inheritable, data[0].inheritable, data[1].inheritable);
        caps
    }

    fn fill(&self, low: &mut Data, high: &mut Data) {
        split(self.effective,   &mut low.effective,   &mut high.effective);
        split(self.permitted,   &mut low.permitted,   &mut high.permitted);
        split(self.inheritable, &mut low.inheritable, &mut high.inheritable);
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
struct Header {
    version: u32,
    pid:     c_int,
}

#[derive(Clone, Debug, Default)]
#[repr(C)]
struct Data {
    effective:   u32,
    permitted:   u32,
    inheritable: u32,
}

impl Header {
    fn new(pid: Option<c_int>) -> Self {
        Self {
            version: LINUX_CAPABILITY_VERSION_3,
            pid:     pid.unwrap_or(0),
        }
    }
}

fn split(n: u64, low: &mut u32, high: &mut u32) {
    *low  = u32::try_from(n >> 00 & 0xFFFFFFFF).unwrap_or(0);
    *high = u32::try_from(n >> 32 & 0xFFFFFFFF).unwrap_or(0);
}

fn join(n: &mut u64, low: u32, high: u32) {
    *n = u64::from(high) << 32 | u64::from(low)
}

fn capget(pid: Option<c_int>) -> Result<Caps> {
    let head = Header::new(pid);

    let mut data: [Data; 2] = Default::default();
    unsafe {
        match syscall(SYS_capget, &head, &mut data) {
            0 => Ok(Caps::new(&data)),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

fn capset(pid: Option<c_int>, caps: &Caps) -> Result<()> {
    let head = Header::new(pid);

    let mut low  = Data::default();
    let mut high = Data::default();
    caps.fill(&mut low, &mut high);

    unsafe {
        match syscall(SYS_capset, &head, &[low, high]) {
            0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

fn set_securebits(bits: c_int) -> Result<()> {
    unsafe {
        match prctl(PR_SET_SECUREBITS, bits) {
            0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

#[allow(unused)]
fn set_keepcaps() -> Result<()> {
    unsafe {
        match prctl(PR_SET_KEEPCAPS, 1) {
            0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

const LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;
const CAP_NET_RAW:                u64 = 1 << 13;

const SECURE_KEEP_CAPS: c_int = 4;
const SECBIT_KEEP_CAPS: c_int = 1 << SECURE_KEEP_CAPS;
