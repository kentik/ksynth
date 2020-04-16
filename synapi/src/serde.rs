use std::fmt::Display;
use std::str::FromStr;
use serde::{Deserialize, de::{Deserializer, Error}};

pub fn id<'d, D: Deserializer<'d>, T: FromStr>(de: D) -> Result<T, D::Error> where T::Err: Display {
    let s = <&str>::deserialize(de)?;
    T::from_str(s).map_err(Error::custom)
}
