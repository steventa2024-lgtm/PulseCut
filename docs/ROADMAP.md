# Roadmap

Each phase must leave a working, tested editor. Scope is described by components touched and risk, not by dates.

## Phase 0 — audit, architecture, engine proof ✅

- Environment inventory, engine spike (MLT 7.22 vs FFmpeg graph) with measurements → [ENGINE_EVALUATION.md](ENGINE_EVALUATION.md)
- Project data model, time base, serialization and migration plan → [ARCHITECTURE.md](ARCHITECTURE.md), ADR 0002
- License decision and dependency audit → [LICENSE_AUDIT.md](LICENSE_AUDIT.md), ADR 0003
- Preview/export invariants and acceptance tests → ARCHITECTURE.md

## Phase 1 — first usable editor (current: implemented on Linux; Windows verification pending)

Done: import → metadata → drag to tracks → engine-rendered preview and synced playback → frame step/seek → trim/split/move/delete → undo/redo → save/reopen → export MP4 → automatic verification. Autosave recovery, relink, keyboard shortcuts, resizable layout.

**Remaining to close Phase 1:**

1. **Windows 11 evidence.** CI job green (build + engine tests with FFmpeg), then a manual run of the GUI checklist on a real Windows 11 machine, including NSIS installer install/uninstall. Risk: WebView2 MJPEG/`createImageBitmap` throughput, `CREATE_NO_WINDOW` behaviour, path handling with non-ASCII characters.
2. **Performance baseline** on a documented Windows machine and an RTX 3070 Ti 8 GB-class system: playback drop rate at 1080p30 (½ and full preview), still latency, export speed, peak RAM.
3. **FFmpeg distribution decision**: user-installed (current) vs pinned bundled build with source offer.
4. A VFR fixture and a rotated-phone fixture with tests.

## Phase 2 — editing essentials

Components: project-model v2 (schema migration 1 → 2), plan compiler, UI.

- Insert/overwrite modes, ripple/roll/slip/slide, markers and in/out ranges, unlink/relink A/V, multi-clip move
- Transitions (`xfade`/`acrossfade`) and audio fades — requires overlap model on a track or transition objects
- Keyframes for transform/opacity/gain (rational time keys, interpolation) — expressed as FFmpeg expressions or via engine switch (re-evaluate MLT here)
- Titles (libass or `drawtext`), crop/rotate, basic effects
- Multiple sequences and nesting
- Persistent thumbnail/waveform cache, proxy generation job queue, persistent frame cache for scrubbing
- Export presets: HEVC, AV1, VP9/WebM, audio-only
- Regression fixtures: sample projects in `fixtures/projects/` with golden render hashes

Risk: per-clip decoder count grows with transitions; segment-based rendering likely needed.

## Phase 3 — professional toolset

Color (lift/gamma/gain, LUTs), scopes, masks/keying, tracking, stabilization, retiming, multicam, EQ/dynamics/loudness, captions (SRT/VTT, burn-in), hardware decode/encode, OTIO/EDL interchange, extension interfaces (data-only, no executable plugins).

## Phase 4 — advanced / optional AI

Node compositor workspace, motion-graphics templates, opt-in offline transcription (whisper.cpp with user-downloaded models sized for 8 GB VRAM), text-based editing, documented permissioned scripting API, Linux/macOS packaging.

## Phase 5 — public release quality

Signed Windows installer, SBOM + generated third-party notices, `cargo deny` gate, security review, reproducible release build instructions, benchmarks, real screenshots/video, issue templates, changelog.
