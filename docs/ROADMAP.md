# Roadmap

Phases add tested behavior to an editor that already renders. They are not a claim that the later list exists.

## Phase 0 — done in this tree

Architecture, design tokens, engine measurements, license audit, ADRs, and the project model are in `docs/` and `crates/project-model`. The engine decision is FFmpeg as a subprocess.

## Phase 1 — in progress

The vertical slice is import, a real timeline, proxy playback, trim/split/move/delete, undo, save/reopen, and an H.264 export.

Done and tested without the GUI: the model, the filter graph, and `media-engine` pipeline tests.

Present in the Tauri shell and not yet clicked through on a display: menus, browser, monitor, inspector, timeline, export dialog, recovery banner, sample timeline.

Not done, so Phase 1 is not complete:

- A recorded GUI pass of launch → sample or import → play → trim → save → reopen → export
- A Windows 11 build
- Proof that file dialogs work where the desktop portal is broken

Next smallest milestone: run `pnpm tauri dev` from `apps/desktop` on a machine with FFmpeg, a working display, and a working file dialog; load the sample; play; split; save; reopen; export; ffprobe the file. Record the log. Do not call the MVP done before that pass.

## Phase 2

Transitions, a small set of effects with the same graph in preview and export, titles, keyframes, image overlays, nested sequences, audio fades, a simple mixer, and a proxy workflow. Each one needs a fixture test before it is marked Tested.

## Phase 3

Grading and scopes, masks and keying, tracking, retiming, multicam, caption import/export, interchange that names unsupported mappings, and a measured GPU path.

## Phase 4

Motion-graphics templates, a node workspace that still writes the same project model, and optional local models. Models stay off the export path and are not downloaded unless the user asks.

## Phase 5

CI beyond the current lint and test workflow, a Windows artifact, an SBOM, notices for any bundled FFmpeg, and release notes that list only Tested or Released rows from the feature matrix.
