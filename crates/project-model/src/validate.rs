use crate::model::*;
use crate::time::Rational;
use std::collections::HashSet;

/// Structural invariants every committed or loaded project must satisfy.
pub fn validate_project(p: &Project) -> Result<(), String> {
    if p.format != FILE_FORMAT {
        return Err(format!("not a PulseCut project (format \"{}\")", p.format));
    }
    let seq = &p.sequence;
    if !seq.frame_rate.is_positive() || seq.frame_rate > Rational::from_int(240) {
        return Err(format!("unsupported sequence frame rate {}", seq.frame_rate));
    }
    if !(16..=8192).contains(&seq.width) || !(16..=8192).contains(&seq.height) || seq.width % 2 != 0 || seq.height % 2 != 0 {
        return Err(format!("unsupported sequence size {}x{} (even values 16–8192 required)", seq.width, seq.height));
    }
    if !(8000..=192_000).contains(&seq.sample_rate) {
        return Err(format!("unsupported sample rate {}", seq.sample_rate));
    }

    let mut ids = HashSet::new();
    for a in &p.assets {
        if !ids.insert(a.id) {
            return Err(format!("duplicate asset id {}", a.id));
        }
        if a.path.contains('\0') {
            return Err("asset path contains a NUL byte".into());
        }
    }
    for t in &seq.tracks {
        if !ids.insert(t.id) {
            return Err(format!("duplicate track id {}", t.id));
        }
        let mut prev_end = i64::MIN;
        for c in &t.clips {
            if !ids.insert(c.id) {
                return Err(format!("duplicate clip id {}", c.id));
            }
            let asset = p.asset(c.asset_id).ok_or_else(|| format!("clip {} references a missing asset", c.id))?;
            if c.start < 0 || c.duration <= 0 {
                return Err(format!("clip {} has an invalid range", c.id));
            }
            if c.start < prev_end {
                return Err(format!("clips overlap on track {}", t.name));
            }
            prev_end = c.end();
            if c.source_in < Rational::ZERO {
                return Err(format!("clip {} has a negative in-point", c.id));
            }
            if let Some(avail) = seq.available_frames(asset, c.source_in) {
                if c.duration > avail {
                    return Err(format!("clip {} extends past the end of \"{}\"", c.id, asset.name));
                }
            }
            let ok_kind = match t.kind {
                TrackKind::Video => asset.media.video.is_some(),
                TrackKind::Audio => asset.media.audio.is_some(),
            };
            if !ok_kind {
                return Err(format!("\"{}\" cannot be on {} track {}", asset.name, if t.kind == TrackKind::Video { "video" } else { "audio" }, t.name));
            }
            c.params.validate()?;
        }
    }
    Ok(())
}
