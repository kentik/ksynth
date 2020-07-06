use std::convert::TryFrom;
use std::num::TryFromIntError;
use std::time::Duration;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Summary {
    pub min: Duration,
    pub max: Duration,
    pub avg: Duration,
    pub std: Duration,
    pub jit: Duration,
}

pub fn summarize(ds: &[Duration]) -> Option<Summary> {
    let (usecs, mean) = convert(ds)?;

    let mut iter = usecs.into_iter();
    let count = iter.len();
    let first = iter.next()?;

    let mut min = first;
    let mut max = first;
    let mut sum = (first - mean).pow(2);
    let mut dif = 0.0;
    let mut last = first;

    for us in iter {
        min = us.min(min);
        max = us.max(max);
        sum += (us - mean).pow(2);
        dif += f64::from(us - last).abs();
        last = us;
    }

    let count = i32::try_from(count).ok()?;
    let stdev = f64::from(sum.checked_div(count)?).sqrt();
    let stdev = stdev.round() as i32;
    let mut jit = 0;
    if count > 1 {
        jit = f64::from(dif/f64::from(count-1)).round() as i32;
    }

    Some(Summary {
        min: Duration::from_micros(u64::try_from(min).ok()?),
        max: Duration::from_micros(u64::try_from(max).ok()?),
        avg: Duration::from_micros(u64::try_from(mean).ok()?),
        std: Duration::from_micros(u64::try_from(stdev).ok()?),
        jit: Duration::from_micros(u64::try_from(jit).ok()?),
    })
}

fn convert(ds: &[Duration]) -> Option<(Vec<i32>, i32)> {
    let mut sum = 0;

    let micros = ds.iter().map(|d| {
        let us = micros(d)?;
        sum += us;
        Ok(us)
    }).collect::<Result<Vec<_>, TryFromIntError>>().ok()?;

    let count = i32::try_from(micros.len()).ok()?;
    let mean  = sum.checked_div(count)?;

    Some((micros, mean))
}

fn micros(d: &Duration) -> Result<i32, TryFromIntError> {
    i32::try_from(i128::try_from(d.as_micros())?)
}

#[cfg(test)]
mod test {
    use std::time::Duration;
    use super::Summary;

    #[test]
    fn invariants() {
        let zero = Duration::from_secs(0);
        let one  = Duration::from_secs(1);
        let max  = Duration::from_secs(u64::from(u32::MAX));

        assert_eq!(None, super::summarize(&[]));
        assert_eq!(None, super::summarize(&[max]));

        assert_eq!(Some(Summary {
            min: one,
            max: one,
            avg: one,
            std: zero,
            jit: zero,
        }), super::summarize(&[one]));
    }

    #[test]
    fn summarize() {
        let result = super::summarize(&[
            Duration::from_micros(100),
            Duration::from_micros(200),
            Duration::from_micros(300),
            Duration::from_micros(300),
        ]);

        assert_eq!(Some(Summary {
            min: Duration::from_micros(100),
            max: Duration::from_micros(300),
            avg: Duration::from_micros(225),
            std: Duration::from_micros(83),
            jit: Duration::from_micros(67),
        }), result);
    }
}
