# ADR 0002 — Time base and project model

- Status: accepted (2026-10-12)

## Decision

- Timeline positions and durations are **integer frames in the sequence frame rate**.
- Source in-points are **exact rational seconds** (`Rational`, normalized `i64/i64`, `i128` intermediates).
- Sequence frame rate is a rational (`30000/1001` etc.). Timecode display is non-drop-frame.
- The canonical model lives in Rust (`crates/project-model`). The UI only sends `EditCommand`s and receives the whole project back.
- Undo/redo stores full project snapshots.
- `.pulsecut` is versioned JSON (`schemaVersion`), written atomically, with absolute + project-relative media paths.

## Why

- Integer frames make snapping, trimming and overlap checks exact and simple; editors operate on frames.
- Rational source time avoids drift with mixed rates (25 fps clip on a 30 fps sequence) because source seconds are computed as `source_in + frames × den/num` without rounding.
- Snapshot undo guarantees exact restoration (verified by property tests) and is cheap at MVP project sizes (kilobytes per step).

## Consequences

- Sub-frame audio edits are not representable on the timeline (audio clips start on frame boundaries). Sample-accurate audio offsets can be added as a per-clip field in a later schema.
- Changing a sequence's frame rate after editing will need an explicit conversion command (not implemented).
- Snapshot memory grows with project size; switch to structural sharing or inverse commands if projects grow to thousands of clips.
- Variable-frame-rate sources are conformed by the `fps` filter at render time; their source time is still exact.
