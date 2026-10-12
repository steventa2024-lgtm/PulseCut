use serde::{Deserialize, Serialize};

use crate::ProjectError;

/// Frames per second as an exact ratio. `30/1` is 30 fps. `30000/1001` is NTSC 29.97.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rational {
    pub num: u32,
    pub den: u32,
}

impl Rational {
    pub const US: i64 = 1_000_000;

    pub fn new(num: u32, den: u32) -> Result<Self, ProjectError> {
        if num == 0 || den == 0 {
            return Err(ProjectError::msg(
                "Frame rate numerator and denominator must both be greater than zero.",
            ));
        }
        let fps = f64::from(num) / f64::from(den);
        if !(1.0..=120.0).contains(&fps) {
            return Err(ProjectError::msg(
                "Frame rate must be between 1 and 120 frames per second.",
            ));
        }
        Ok(Self { num, den })
    }

    /// First microsecond of `frame`, using integer floor so timestamps do not drift ahead.
    pub fn frame_start_us(self, frame: i64) -> i64 {
        if frame <= 0 {
            return 0;
        }
        let numer = i128::from(frame) * i128::from(Self::US) * i128::from(self.den);
        let denom = i128::from(self.num);
        (numer / denom) as i64
    }

    /// Frame that contains `time_us` (floor). Negative times belong to frame 0.
    pub fn frame_index(self, time_us: i64) -> i64 {
        if time_us <= 0 {
            return 0;
        }
        let numer = i128::from(time_us) * i128::from(self.num);
        let denom = i128::from(Self::US) * i128::from(self.den);
        (numer / denom) as i64
    }

    pub fn snap_floor_us(self, time_us: i64) -> i64 {
        self.frame_start_us(self.frame_index(time_us.max(0)))
    }

    pub fn min_clip_us(self) -> i64 {
        self.frame_start_us(1).max(1)
    }
}

pub fn format_seconds(us: i64) -> String {
    let us = us.max(0);
    let whole = us / Rational::US;
    let frac = us % Rational::US;
    format!("{whole}.{frac:06}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_fps_second_boundary_is_exact() {
        let fps = Rational::new(30, 1).unwrap();
        assert_eq!(fps.frame_index(1_000_000), 30);
        assert_eq!(fps.frame_start_us(30), 1_000_000);
        assert_eq!(fps.frame_index(999_999), 29);
        assert_eq!(fps.snap_floor_us(999_999), fps.frame_start_us(29));
    }

    #[test]
    fn ntsc_rate_does_not_run_ahead_over_one_hour() {
        let fps = Rational::new(30_000, 1_001).unwrap();
        let hour_us = 3_600 * Rational::US;
        let frame = fps.frame_index(hour_us);
        let start = fps.frame_start_us(frame);
        let next = fps.frame_start_us(frame + 1);
        assert!(start <= hour_us);
        assert!(next > hour_us);
        // 3600 * 30000/1001 = 107892.107... frames.
        assert!((107_892..=107_893).contains(&frame));
    }

    #[test]
    fn random_times_stay_inside_their_frame() {
        let fps = Rational::new(24, 1).unwrap();
        let mut state = 0x1234_5678_u64;
        for _ in 0..2_000 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let time = (state % (30 * 1_000_000)) as i64;
            let frame = fps.frame_index(time);
            let start = fps.frame_start_us(frame);
            let next = fps.frame_start_us(frame + 1);
            assert!(
                start <= time && time < next,
                "{time} not in [{start}, {next})"
            );
        }
    }
}
