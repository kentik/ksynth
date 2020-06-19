use anyhow::Result;
use super::setuser;

pub fn apply(user: Option<&str>) -> Result<()> {
    user.map(setuser).transpose()?;
    Ok(())
}
