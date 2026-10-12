# ADR 0003 — Microseconds and a rational frame rate

Status: accepted

## Context

Clips arrive at different frame rates, including 30000/1001 and variable frame rate. Float seconds drift. A single integer frame index cannot represent a 24 fps source on a 30 fps sequence without resampling the source.

## Decision

Store media and timeline positions as integer microseconds. Store the sequence rate as `fpsNum/fpsDen`. Snap timeline positions to that grid with integer floor division. Keep source in and out in microseconds. Display non-drop timecode. Flag variable frame rate and HDR. Do not drop-frame the display in this version.

## Consequences

`crates/project-model/src/time.rs` tests the 30 fps second boundary, a one-hour 30000/1001 span, and random times that must stay inside their frame. Audio sample rates are not the timeline clock. The renderer resamples audio to 48 kHz at export time.
