# Contributing

PulseCut is GPL-3.0-or-later. Contributions are accepted under that license.

## Setup

Install Rust 1.99.0 (the `rust-toolchain.toml` pin), Node 22, pnpm 10, and FFmpeg with libx264 and AAC. On Linux, the Tauri shell also needs the WebKitGTK packages Tauri 2 documents for the host distro.

```bash
cargo test --workspace
cd apps/desktop
pnpm install
pnpm test
pnpm exec tsc --noEmit
pnpm tauri dev
```

`scripts/windows-dev.ps1` lists the same tools for Windows. It has not been executed in the environment that added the first slice.

## Rules that keep the ledger honest

- A control that does not edit, play, or export does not ship.
- Mark new behavior in `docs/FEATURE_MATRIX.md` as Prototype, Implemented, or Tested. Use Tested only after a test in this repo has passed.
- Do not claim parity with another product.
- Do not add a cloud account, a paid key, or telemetry.
- Preview and export of the same timeline go through `media_engine::build_plan`.
- Schema changes bump `SCHEMA_VERSION` and need a migration test.
- Do not vendor FFmpeg or other codec libraries unless the license audit is updated in the same change.

## Tests

`cargo test -p project-model` covers time, undo, and edits. `cargo test -p media-engine` covers the graph and the render pipeline. The pipeline needs FFmpeg on `PATH` and writes only under the temp directory. `pnpm test` in `apps/desktop` covers timecode helpers.
