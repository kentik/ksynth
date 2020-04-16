use serde::{Deserialize, de::{Deserializer, Error, Unexpected}};

#[derive(Debug)]
pub enum Auth {
    Ok(String),
    Wait,
    Deny,
}

impl<'d> Deserialize<'d> for Auth {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        #[derive(Debug, Deserialize)]
        struct AuthContainer<'d> {
            status:  u64,
            msg:     &'d str,
            session: Option<String>,
        }

        let mut c  = AuthContainer::deserialize(de)?;
        let status = c.status;

        match status  {
            0 => Ok(Auth::Ok(c.session.take().ok_or(D::Error::missing_field("session"))?)),
            1 => Ok(Auth::Wait),
            3 => Ok(Auth::Deny),
            n => Err(Error::invalid_value(Unexpected::Unsigned(n), &"0,1,3")),
        }
    }
}
