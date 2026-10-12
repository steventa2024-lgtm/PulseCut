# ADR 0004 — The Rust project is the source of truth

Status: accepted

## Context

A timeline that the UI mutates independently will disagree with the renderer and the saved file.

## Decision

`project_model::Session` is the only editor state that can be saved. The React layer mirrors `UiState` returned by every command. Drag ghosts are local until pointer-up dispatches `moveClip` or `trimClip`. Undo stores full project snapshots. Schema version 1 is rejected if a file claims another version. There is no migrator yet because there is no version 0 in the wild.

## Consequences

The desktop shell must stay up for an edit to exist. A second process cannot share the session. Crash recovery is a separate autosave file, not the undo stack.
