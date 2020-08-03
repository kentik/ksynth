use std::fmt;
use bytes::Bytes;
use serde::{Serialize, Deserialize};
use super::time::Timestamp;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Artifact {
    pub name:      String,
    pub version:   Version,
    pub target:    Target,
    pub signature: Bytes,
    pub timestamp: Timestamp,
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Version {
    major: u16,
    minor: u16,
    patch: u16,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Target {
    pub arch:   Arch,
    pub system: System,
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum Arch {
    X86_64,
    AArch64,
}

#[derive(Copy, Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum System {
    Linux,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.arch, self.system)
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Arch::X86_64  => f.write_str(X86_64),
            Arch::AArch64 => f.write_str(AARCH64),
        }
    }
}

impl fmt::Display for System {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            System::Linux => f.write_str(LINUX),
        }
    }
}
const X86_64:  &str = "x86_64";
const AARCH64: &str = "aarch64";
const LINUX:   &str = "linux";
