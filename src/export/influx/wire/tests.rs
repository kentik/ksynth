use anyhow::Result;
use super::{Point, Field, Tag, Value};

#[test]
fn write_point() -> Result<()> {
    let field = field("value", 3.14);

    let point = Point {
        measurement: "point".into(),
        tags:        &[],
        fields:      &[field],
        timestamp:   1234,
    };

    let expect = "point value=3.14 1234\n";

    assert_eq!(expect, point.to_string());

    Ok(())
}

#[test]
fn write_tags() -> Result<()> {
    let tag0  = tag("foo", "A");
    let tag1  = tag("bar", "B");
    let field = field("value", 3.14);

    let point = Point {
        measurement: "point".into(),
        tags:        &[tag0, tag1],
        fields:      &[field],
        timestamp:   1234,
    };

    let expect = "point,foo=A,bar=B value=3.14 1234\n";

    assert_eq!(expect, point.to_string());

    Ok(())
}

#[test]
fn write_fields() -> Result<()> {
    let f0 = field("foo", 3.14);
    let f1 = field("bar", 6.28);

    let point = Point {
        measurement: "point".into(),
        tags:        &[],
        fields:      &[f0, f1],
        timestamp:   1234,
    };

    let expect = "point foo=3.14,bar=6.28 1234\n";

    assert_eq!(expect, point.to_string());

    Ok(())
}

#[test]
fn write_value() -> Result<()> {
    fn check<'a, T: Into<Value<'a>>>(expect: &str, v: T) {
        let mut vec = Vec::new();
        v.into().write(&mut vec);
        let actual = String::from_utf8(vec).unwrap();
        assert_eq!(expect, actual);
    }

    check("3.141", 3.141f64);
    check("-1.23", -1.23f64);
    check("-456i", -456i16);
    check("7890u", 7890u32);
    check("true",  true);
    check("false", false);
    check("\"A\"", "A");

    Ok(())
}

fn field<'a>(key: &'a str, val: impl Into<Value<'a>>) -> Field<'a> {
    Field {
        key:   key.into(),
        value: val.into(),
    }
}

fn tag<'a>(key: &'a str, val: &'a str) -> Tag<'a> {
    Tag {
        key:   key.into(),
        value: val.into(),
    }
}
