#![allow(unused)]

use std::ffi::CString;
use std::io::Error;
use std::mem::zeroed;
use anyhow::{Result, anyhow};
use libc::{self, gid_t, uid_t, passwd};

pub fn setuser(name: &str) -> Result<uid_t> {
    let (gid, uid) = getpwnam(name)?;

    setgid(gid)?;
    setuid(uid)?;

    Ok(uid)
}

fn getpwnam(name: &str) -> Result<(gid_t, uid_t)> {
    let cstr = CString::new(name)?;
    unsafe {
        let mut pwd = zeroed::<passwd>();
        let mut ptr = &mut pwd as *mut _;
        let mut buf = [0; 512];

        libc::getpwnam_r(cstr.as_ptr(), &mut pwd, buf.as_mut_ptr(), buf.len(), &mut ptr);

        match ptr.is_null() {
            false => Ok((pwd.pw_gid, pwd.pw_uid)),
            true  => Err(anyhow!("invalid user {}", name)),
        }
    }
}

pub fn getuid() -> Result<uid_t> {
    unsafe {
        Ok(libc::getuid())
    }
}

fn setgid(gid: gid_t) -> Result<()> {
    unsafe {
        match libc::setgid(gid) {
            0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }
}

fn setuid(uid: uid_t) -> Result<()> {
    unsafe {
        match libc::setuid(uid) {
            0 => Ok(()),
            _ => Err(Error::last_os_error().into()),
        }
    }
}
