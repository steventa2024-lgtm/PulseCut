# ADR 0003 — License PulseCut under GPL-3.0-or-later

- Status: accepted (2026-10-12)
- Evidence: [LICENSE_AUDIT.md](../LICENSE_AUDIT.md)

## Decision

All original PulseCut code and assets are licensed **GPL-3.0-or-later**.

## Why

- Strong copyleft matches the product promise: every distributed build and derivative stays free.
- All resolved Rust and npm dependencies are permissive (MIT/Apache-2.0/BSD/ISC/Zlib/Unicode) or MPL-2.0, all compatible with GPL-3.0. Apache-2.0-only crates (`tao`) are compatible with GPL-3.0 but not GPL-2.0-only, which rules out GPL-2.0-only.
- GPL FFmpeg builds (libx264) are compatible if we ever bundle them.

## Consequences

- Anyone redistributing PulseCut binaries must provide corresponding source.
- Third-party code with GPL-incompatible terms (e.g. some proprietary SDKs, `--enable-nonfree` FFmpeg) can never be bundled.
- Plugin/extension policy must account for GPL linking expectations (decided in Phase 3).
