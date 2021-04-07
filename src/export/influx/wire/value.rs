#[derive(Clone, Debug, PartialEq)]
pub enum Value<'a> {
    Float(f64),
    Int(i64),
    UInt(u64),
    Str(&'a str),
    Bool(bool),
}

impl<'a> Value<'a> {
    pub fn write(&self, vec: &mut Vec<u8>) {
        match self {
            Self::Float(v)    => f64(vec, *v),
            Self::Int(v)      => i64(vec, *v),
            Self::UInt(v)     => u64(vec, *v),
            Self::Str(v)      => str(vec, v),
            Self::Bool(true)  => vec.extend_from_slice(b"true"),
            Self::Bool(false) => vec.extend_from_slice(b"false"),
        }
    }
}

fn f64(vec: &mut Vec<u8>, n: f64) {
    let mut buf = ryu::Buffer::new();
    vec.extend_from_slice(buf.format(n).as_bytes());
}

fn i64(vec: &mut Vec<u8>, n: i64) {
    let mut buf = itoa::Buffer::new();
    vec.extend_from_slice(buf.format(n).as_bytes());
    vec.push(b'i');
}

fn u64(vec: &mut Vec<u8>, n: u64) {
    let mut buf = itoa::Buffer::new();
    vec.extend_from_slice(buf.format(n).as_bytes());
    vec.push(b'u');
}

fn str(vec: &mut Vec<u8>, s: &str) {
    vec.push(b'"');
    vec.extend_from_slice(s.as_bytes());
    vec.push(b'"');
}

impl<'a> From<f32> for Value<'a> {
    fn from(v: f32) -> Self {
        Self::Float(f64::from(v))
    }
}

impl<'a> From<f64> for Value<'a> {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl<'a> From<i8> for Value<'a> {
    fn from(v: i8) -> Self {
        Self::Int(i64::from(v))
    }
}

impl<'a> From<i16> for Value<'a> {
    fn from(v: i16) -> Self {
        Self::Int(i64::from(v))
    }
}

impl<'a> From<i32> for Value<'a> {
    fn from(v: i32) -> Self {
        Self::Int(i64::from(v))
    }
}

impl<'a> From<i64> for Value<'a> {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl<'a> From<u8> for Value<'a> {
    fn from(v: u8) -> Self {
        Self::UInt(u64::from(v))
    }
}

impl<'a> From<u16> for Value<'a> {
    fn from(v: u16) -> Self {
        Self::UInt(u64::from(v))
    }
}

impl<'a> From<u32> for Value<'a> {
    fn from(v: u32) -> Self {
        Self::UInt(u64::from(v))
    }
}

impl<'a> From<u64> for Value<'a> {
    fn from(v: u64) -> Self {
        Self::UInt(v)
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(v: &'a str) -> Self {
        Self::Str(v.into())
    }
}

impl<'a> From<bool> for Value<'a> {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
