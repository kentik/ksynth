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
        struct AuthContainer<'a> {
            auth:     &'a str,
            #[serde(deserialize_with = "id")]
            agent_id: u64,
            family:   Net,
            session:  Option<String>,
        }

        let AuthContainer {
            auth,
            agent_id,
            family,
            session,
        } = AuthContainer::deserialize(de)?;

        let session = session.ok_or_else(|| Error::missing_field("session"));

        let ok = || Ok((Agent {
            id:  agent_id,
            net: family,
        }, session?));

        match auth  {
            "OK"   => Ok(Auth::Ok(ok()?)),
            "WAIT" => Ok(Auth::Wait),
            "DENY" => Ok(Auth::Deny),
            other  => Err(Error::invalid_value(Unexpected::Str(other), &"OK|WAIT|DENY")),
        }
    }
}
