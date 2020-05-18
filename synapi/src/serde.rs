use std::fmt::Display;
use std::str::FromStr;
use serde::{Deserialize, de::{Deserializer, Error}};
use super::client::{Response, Failure};

pub fn id<'d, D: Deserializer<'d>, T: FromStr>(de: D) -> Result<T, D::Error> where T::Err: Display {
    let s = <&str>::deserialize(de)?;
    T::from_str(s).map_err(Error::custom)
}

#[derive(Debug, Deserialize)]
struct Wrapper<T> {
    status: Status,
    #[serde(flatten)]
    value:  Option<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ok(bool),
    Err(Failure),
}

impl<'d, T: Deserialize<'d>> Deserialize<'d> for Response<T> {
    fn deserialize<D: Deserializer<'d>>(de: D) -> Result<Self, D::Error> {
        let Wrapper { status, value } = Wrapper::<T>::deserialize(de)?;
        let value = || value.ok_or(Error::custom("missing value"));
        Ok(match status {
            Status::Ok(_)     => Response::Success(value()?),
            Status::Err(fail) => Response::Failure(fail),
        })
    }

}
