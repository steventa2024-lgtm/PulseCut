# ADR 0005 — Preview is a half-resolution proxy

Status: accepted

## Context

This machine has no GPU path that was measured, and a realtime compositor was not required to prove edit parity.

## Decision

Play renders the current timeline to an H.264 proxy at half the sequence size and plays that file in a `<video>` element. Audio sync is the file's mux. If the project revision changes, the proxy is marked stale. Export uses the same `build_plan` with export settings. The acceptance bar for this slice is PSNR above 30 dB on the solid-color fixtures, measured in the pipeline test. The Phase 0 spike measured about 50 dB on those colors. That is not a claim about graded camera footage.

## Consequences

The first play after an edit waits for FFmpeg. Scrubbing does not re-render. Cancel and a single-flight render lock keep the UI from stacking encodes.
