use serde::{Deserialize, de::{Deserializer, Error, Unexpected}};

#[derive(Debug)]
pub struct Okay;

impl<'d> Deserialize<'d> for Okay {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        #[derive(Debug, Deserialize)]
        struct Response<'d> {
            status:   u64,
            msg:      &'d str,
        }

        match Response::deserialize(de)?.status  {
            0 => Ok(Okay),
            n => Err(Error::invalid_value(Unexpected::Unsigned(n), &"0")),
        }
    }
}
