# ADR 0001 — FFmpeg subprocess, not MLT

Status: accepted

## Context

The brief asked for a practical proof of MLT plus FFmpeg before the editor was built around either one. Preview and export have to describe the same edits.

## Decision

`crates/media-engine` shells out to `ffmpeg` and `ffprobe`. One filter graph serves preview and export. MLT stays a possible later backend. It is not linked, not required to build, and not described as the engine.

## Evidence

On this VM, FFmpeg wrote a 6.000000 s two-layer file. melt 7.22.0 wrote 5.034 s for the same intended duration. Details are in `docs/ENGINE_EVALUATION.md`.

## Consequences

Users need an FFmpeg build with libx264 and AAC on `PATH`. PulseCut does not redistribute FFmpeg. A live GPU compositor is future work. The program monitor plays a rendered proxy.
