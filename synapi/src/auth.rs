use serde::{Deserialize, de::{Deserializer, Error, Unexpected}};
use crate::serde::id;
use super::agent::{Agent, Net};

#[derive(Debug)]
pub enum Auth {
    Ok((Agent, String)),
    Wait,
    Deny,
}

impl<'d> Deserialize<'d> for Auth {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        #[derive(Debug, Deserialize)]
        struct AuthContainer<'d> {
            status:   u64,
            msg:      &'d str,
            #[serde(deserialize_with = "id")]
            agent_id: u64,
            family:   Net,
            session:  Option<String>,
        }

        let mut c   = AuthContainer::deserialize(de)?;
        let status  = c.status;
        let session = c.session.take();

        let ok = || Ok((Agent {
            id:  c.agent_id,
            net: c.family,
        }, session.ok_or(D::Error::missing_field("session"))?));

        match status  {
            0 => Ok(Auth::Ok(ok()?)),
            1 => Ok(Auth::Wait),
            3 => Ok(Auth::Deny),
            n => Err(Error::invalid_value(Unexpected::Unsigned(n), &"0,1,3")),
        }
    }
}
