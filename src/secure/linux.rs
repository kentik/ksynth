use std::io::Error;
use std::os::raw::c_int;
use anyhow::Result;
use capo::{Ambient, Cap, Caps};
use libc::*;
use nix::unistd::getuid;
use super::setuser;

pub fn apply(user: Option<&str>) -> Result<()> {
    if user.is_some() && getuid().is_root() {
        set_securebits(SECBIT_KEEP_CAPS)?;
    }

    user.map(setuser).transpose()?;

    let mut caps = Caps::empty();
    caps.effective.insert(Cap::NetRaw);
    caps.permitted.insert(Cap::NetRaw);
    caps.inheritable.insert(Cap::NetRaw);
    caps.set()?;

    Ambient::raise(Cap::NetRaw)?;

    Ok(())
}

fn set_securebits(bits: c_int) -> Result<()> {
    unsafe {
        match prctl(PR_SET_SECUREBITS, bits) {
            0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

const SECURE_KEEP_CAPS: c_int = 4;
const SECBIT_KEEP_CAPS: c_int = 1 << SECURE_KEEP_CAPS;
