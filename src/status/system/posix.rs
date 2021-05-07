use anyhow::Result;
use synapi::status::System;

pub fn system() -> Result<System> {
    Ok(System::default())
}
