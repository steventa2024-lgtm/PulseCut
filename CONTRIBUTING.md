# Contributing to PulseCut

Thank you for your interest in contributing. PulseCut is licensed under GPL-3.0-or-later, and every contribution is accepted under that license.

## Ground rules

- **No fake features.** A button or menu item has to do real work, or it doesn't ship. If a feature only partly works, record that in `docs/FEATURE_MATRIX.md`.
- **Preview/export parity is a hard invariant.** Preview, playback, and export all go through the same graph compiler, `media_engine::plan::compile`. Never add a separate code path for preview only. Any change to `plan.rs` has to keep the parity test in `crates/media-engine/tests/end_to_end.rs` passing.
- **Every edit goes through a typed `EditCommand`.** That keeps edits undoable and validated. The UI must never change project state any other way.
- **Original assets only.** Test media are generated synthetically by `scripts/generate-fixtures.sh`. Do not commit third-party footage, icons, or fonts unless their license is compatible and has been added to `docs/LICENSE_AUDIT.md`.
- **Minimum privilege.** Do not add Tauri plugins or capabilities, such as `fs` or `shell`, without an ADR that explains why they are needed.

## Workflow

1. Run `./scripts/test-all.sh` before opening a pull request.
2. For any significant design decision, add an ADR under `docs/DECISIONS/`.
3. Update `docs/FEATURE_MATRIX.md` whenever a feature changes status.
4. New dependencies must have a GPL-3.0-compatible license. Add them to `docs/LICENSE_AUDIT.md`.

## Code style

- Rust: `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`.
- TypeScript: strict mode, `npm run typecheck`. Keep the types in `apps/desktop/src/lib/types.ts` in sync with the Rust model.
