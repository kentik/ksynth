use std::fmt;
use std::str;
use itoa::Buffer;
use super::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Point<'a> {
    pub measurement: &'a str,
    pub tags:        &'a [Tag<'a>],
    pub fields:      &'a [Field<'a>],
    pub timestamp:   u128,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tag<'a> {
    pub key:   &'a str,
    pub value: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field<'a> {
    pub key:   &'a str,
    pub value: Value<'a>
}

impl<'a> Point<'a> {
    pub fn write(&self, vec: &mut Vec<u8>) {
        vec.extend_from_slice(self.measurement.as_bytes());

        for Tag { key, value } in self.tags {
            vec.push(b',');
            vec.extend_from_slice(key.as_bytes());
            vec.push(b'=');
            vec.extend_from_slice(value.as_bytes());
        }
        vec.push(b' ');

        for (index, Field { key, value }) in self.fields.iter().enumerate() {
            if index > 0 { vec.push(b','); }
            vec.extend_from_slice(key.as_bytes());
            vec.push(b'=');
            value.write(vec);
        }
        vec.push(b' ');

        let mut buf = Buffer::new();
        let timestamp = buf.format(self.timestamp);
        vec.extend_from_slice(timestamp.as_bytes());

        vec.push(b'\n');
    }
}

impl<'a> fmt::Display for Point<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut vec = Vec::new();
        self.write(&mut vec);
        let str = str::from_utf8(&vec);
        f.write_str(str.or(Err(fmt::Error))?)
    }
}
