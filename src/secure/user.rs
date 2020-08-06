#![allow(unused)]

use std::ffi::CString;
use std::io::Error;
use std::mem::zeroed;
use anyhow::{Result, anyhow};
use nix::unistd::{User, Uid, setgid, setuid};

pub fn setuser(name: &str) -> Result<Uid> {
    let user = getuser(name)?;

    setgid(user.gid)?;
    setuid(user.uid)?;

    Ok(user.uid)
}

fn getuser(name: &str) -> Result<User> {
    match User::from_name(name)? {
        Some(user) => Ok(user),
        None       => Err(anyhow!("invalid user: {}", name)),
    }
}
