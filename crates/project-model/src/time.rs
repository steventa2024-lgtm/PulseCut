//! Exact rational time.
//!
//! Timeline positions are integer frame indices in the sequence timebase.
//! Source positions (in-points) are exact rational seconds so that clips with a
//! different native frame rate than the sequence never accumulate rounding drift.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// A normalized rational number (`den > 0`, `gcd(num, den) == 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "RationalRepr", into = "RationalRepr")]
pub struct Rational {
    num: i64,
    den: i64,
}

#[derive(Serialize, Deserialize)]
struct RationalRepr {
    num: i64,
    den: i64,
}

impl TryFrom<RationalRepr> for Rational {
    type Error = String;
    fn try_from(r: RationalRepr) -> Result<Self, Self::Error> {
        Rational::try_new(r.num, r.den)
            .ok_or_else(|| format!("invalid rational {}/{}", r.num, r.den))
    }
}

impl From<Rational> for RationalRepr {
    fn from(r: Rational) -> Self {
        RationalRepr {
            num: r.num,
            den: r.den,
        }
    }
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

impl Rational {
    pub const ZERO: Rational = Rational { num: 0, den: 1 };
    pub const HALF: Rational = Rational { num: 1, den: 2 };

    pub fn try_new(num: i64, den: i64) -> Option<Self> {
        if den == 0 {
            return None;
        }
        Self::from_i128(num as i128, den as i128)
    }

    /// Panics if `den == 0`; use [`Rational::try_new`] for untrusted input.
    pub fn new(num: i64, den: i64) -> Self {
        Self::try_new(num, den).expect("rational denominator must be non-zero")
    }

    fn from_i128(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let g = gcd(num, den);
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            n = -n;
            d = -d;
        }
        Some(Rational {
            num: i64::try_from(n).ok()?,
            den: i64::try_from(d).ok()?,
        })
    }

    pub fn from_int(v: i64) -> Self {
        Rational { num: v, den: 1 }
    }

    pub fn num(self) -> i64 {
        self.num
    }

    pub fn den(self) -> i64 {
        self.den
    }

    pub fn as_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub fn is_positive(self) -> bool {
        self.num > 0
    }

    pub fn checked_add(self, o: Rational) -> Option<Rational> {
        Self::from_i128(
            self.num as i128 * o.den as i128 + o.num as i128 * self.den as i128,
            self.den as i128 * o.den as i128,
        )
    }

    pub fn checked_sub(self, o: Rational) -> Option<Rational> {
        self.checked_add(Rational {
            num: -o.num,
            den: o.den,
        })
    }

    pub fn checked_mul(self, o: Rational) -> Option<Rational> {
        Self::from_i128(
            self.num as i128 * o.num as i128,
            self.den as i128 * o.den as i128,
        )
    }

    pub fn checked_div(self, o: Rational) -> Option<Rational> {
        Self::from_i128(
            self.num as i128 * o.den as i128,
            self.den as i128 * o.num as i128,
        )
    }

    pub fn add(self, o: Rational) -> Rational {
        self.checked_add(o).expect("rational overflow")
    }

    pub fn sub(self, o: Rational) -> Rational {
        self.checked_sub(o).expect("rational overflow")
    }

    /// Parses FFmpeg-style `"30000/1001"` or plain integers/decimals like `"29.97"`.
    pub fn parse(s: &str) -> Option<Rational> {
        let s = s.trim();
        if let Some((a, b)) = s.split_once('/') {
            return Rational::try_new(a.trim().parse().ok()?, b.trim().parse().ok()?);
        }
        if let Some((int, frac)) = s.split_once('.') {
            let digits = frac.len().min(9) as u32;
            let frac = &frac[..digits as usize];
            let den = 10i64.pow(digits);
            let int_v: i64 = if int.is_empty() || int == "-" {
                0
            } else {
                int.parse().ok()?
            };
            let frac_v: i64 = if frac.is_empty() {
                0
            } else {
                frac.parse().ok()?
            };
            let sign = if s.starts_with('-') { -1 } else { 1 };
            return Rational::try_new(int_v * den + sign * frac_v, den);
        }
        Some(Rational::from_int(s.parse().ok()?))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.num as i128 * other.den as i128).cmp(&(other.num as i128 * self.den as i128))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

/// Exact duration in seconds of `frames` frames at `fps`.
pub fn frames_to_seconds(frames: i64, fps: Rational) -> Rational {
    Rational::from_i128(frames as i128 * fps.den as i128, fps.num as i128)
        .expect("fps must be non-zero")
}

/// Number of whole frames at `fps` that fit in `seconds` (floor), with a tolerance
/// of 1/1000 of a frame so that e.g. 2.0 s at 30 fps is exactly 60, not 59.
pub fn seconds_to_frames_floor(seconds: Rational, fps: Rational) -> i64 {
    let n = seconds.num as i128 * fps.num as i128;
    let d = seconds.den as i128 * fps.den as i128;
    let scaled = n * 1000 + d;
    scaled.div_euclid(d * 1000) as i64
}

/// Non-drop-frame SMPTE-style timecode `HH:MM:SS:FF` for a frame index.
/// For fractional rates (e.g. 29.97) the nominal integer rate (30) is used,
/// matching non-drop-frame convention; drop-frame display is not implemented.
pub fn format_timecode(frame: i64, fps: Rational) -> String {
    let nominal = ((fps.num + fps.den - 1) / fps.den).max(1);
    let sign = if frame < 0 { "-" } else { "" };
    let f = frame.abs();
    let ff = f % nominal;
    let total_s = f / nominal;
    format!(
        "{sign}{:02}:{:02}:{:02}:{:02}",
        total_s / 3600,
        (total_s / 60) % 60,
        total_s % 60,
        ff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(Rational::new(60, 2), Rational::from_int(30));
        assert_eq!(Rational::new(1, -2), Rational::new(-1, 2));
        assert!(Rational::try_new(1, 0).is_none());
    }

    #[test]
    fn parses_ffmpeg_rates() {
        assert_eq!(
            Rational::parse("30000/1001"),
            Some(Rational::new(30000, 1001))
        );
        assert_eq!(Rational::parse("25"), Some(Rational::from_int(25)));
        assert_eq!(Rational::parse("4.010000"), Some(Rational::new(401, 100)));
        assert_eq!(Rational::parse("0/0"), None);
    }

    #[test]
    fn frame_conversions_are_exact() {
        let ntsc = Rational::new(30000, 1001);
        assert_eq!(frames_to_seconds(30000, ntsc), Rational::from_int(1001));
        assert_eq!(
            seconds_to_frames_floor(Rational::from_int(2), Rational::from_int(30)),
            60
        );
        assert_eq!(
            seconds_to_frames_floor(Rational::new(1, 3), Rational::from_int(30)),
            10
        );
        assert_eq!(
            seconds_to_frames_floor(Rational::new(4, 100), Rational::from_int(25)),
            1
        );
    }

    #[test]
    fn timecode() {
        assert_eq!(format_timecode(0, Rational::from_int(30)), "00:00:00:00");
        assert_eq!(
            format_timecode(30 * 3661 + 7, Rational::from_int(30)),
            "01:01:01:07"
        );
        assert_eq!(
            format_timecode(31, Rational::new(30000, 1001)),
            "00:00:01:01"
        );
    }
}
