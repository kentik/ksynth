use anyhow::Result;
use nix::sys::sysinfo::sysinfo;
use procfs::{CpuPressure, IoPressure, MemoryPressure};
use synapi::status::System;

pub fn system() -> Result<System> {
    let load = sysinfo()?.load_average();
    let psi  = pressure().unwrap_or_default();

    Ok(System {
        load: load.0,
        cpu:  psi.cpu,
        io:   psi.io,
        mem:  psi.mem,
    })
}

#[derive(Default)]
pub struct Pressure {
    pub cpu: f32,
    pub io:  f32,
    pub mem: f32,
}

fn pressure() -> Result<Pressure> {
    let cpu = CpuPressure::new()?;
    let io  = IoPressure::new()?;
    let mem = MemoryPressure::new()?;
    Ok(Pressure {
        cpu: cpu.some.avg60,
        io:  io.some.avg60,
        mem: mem.some.avg60,
    })
}
