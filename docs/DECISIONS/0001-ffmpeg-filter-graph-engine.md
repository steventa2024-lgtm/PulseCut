# ADR 0001 — FFmpeg filter-graph render backend for the MVP

- Status: accepted (2026-10-12)
- Context: [ENGINE_EVALUATION.md](../ENGINE_EVALUATION.md)

## Decision

The `media-engine` crate compiles the PulseCut project model into a single FFmpeg `-filter_complex` graph per render request and runs FFmpeg as a child process. The same compiler serves preview stills, real-time playback and export.

## Why

- Both MLT 7.22 and the FFmpeg graph rendered the spike timeline correctly; the FFmpeg graph was faster (0.29 s vs ~0.7 s for 4 s), produced exact duration (4.000 s vs 4.010 s) and no muxer timestamp warnings.
- Packaging on Windows is one or two executables instead of an MSYS2-built MLT tree, and PulseCut needs no C FFI.
- PulseCut's own model is the source of truth; a pure `compile()` function is unit-testable and gives preview/export parity by construction.

## Consequences

- Transitions, keyframes and effects must be expressed by PulseCut as FFmpeg filter expressions. If that becomes unwieldy (Phase 2), re-evaluate MLT behind the same crate boundary, starting with a Windows packaging proof.
- Each still costs a process start + seek (~100–170 ms measured on Linux). A persistent decoder/frame cache is required for smooth scrubbing later.
- Requires FFmpeg ≥ 5.0 filter syntax.
