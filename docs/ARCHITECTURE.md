# PulseCut architecture

PulseCut is a local desktop editor. The timeline the user edits, the file they save, the preview they watch, and the file they export are four views of one project document.

## Boundaries

| Piece | Crate / app | Responsibility |
| --- | --- | --- |
| Project model | `crates/project-model` | Schema, time math, undoable edits, JSON |
| Media engine | `crates/media-engine` | Probe, filter graph, FFmpeg render, thumbnails, waveforms |
| Desktop shell | `apps/desktop/src-tauri` | IPC, paths, recovery, one render at a time |
| UI | `apps/desktop/src` | Layout, transport, timeline gestures |

There is no `app-services` crate and no shared `packages/ui` package in this slice. Cache, recovery, and jobs live in the desktop command module until a second client needs them.

The UI does not compose frames. It asks Rust to render a proxy, then plays that file. Export uses the same graph builder with different settings.

## Project document

- Extension: `.pulsecut`
- Encoding: pretty JSON, schema version `1`, camelCase fields
- IDs: UUID strings
- Media times: integer microseconds
- Sequence rate: `fpsNum` / `fpsDen`
- A new project has video tracks V1 and V2 and audio tracks A1 and A2. V1 is underneath V2.
- Assets store an absolute path and, after save, a path relative to the project file. Load resolves the relative path and marks the asset offline when the file is missing.
- `http`, `https`, `ftp`, and `rtsp` paths are rejected.
- History is not stored in the file. Undo is a session stack of full project snapshots, capped at 100. Dirty means the canonical JSON differs from the last saved snapshot.

`prepare_for_save` rewrites relative paths. `resolve_after_load` restores absolute paths. Save writes a temporary file and renames it into place.

## Time

`Rational::frame_index` is `floor(time_us * num / (1_000_000 * den))`. `frame_start_us` uses integer division so a timestamp never lands past the frame it names. Timeline edits snap to the sequence frame grid. Source in and out stay in microseconds so a 24 fps clip on a 30 fps sequence is not resampled in the document. The shortest clip is one sequence frame.

Displayed timecode is non-drop `HH:MM:SS:FF` at the rounded nominal rate. Drop-frame display is not implemented. Mixed rates, variable frame rate, and HDR are recorded on the asset (`vfrSuspected`, `hdr`) and shown as warnings. They are not conformed.

Audio is mixed at 48 kHz stereo in the filter graph. Gain is decibels on audio clips only, clamped from -60 to 12. Linear gain is `10^(dB/20)`.

## Edits

`Session::dispatch` clones the project, applies one command, validates, and commits. A failed command leaves the previous project in place. Linked clips share `linkId` and move, trim, split, and delete together. Unlink clears that group. Placing a video asset can create the linked audio clip in the same undo step.

Commands are JSON with a `type` tag. The UI sends that JSON over Tauri IPC. The desktop crate does not shell out to a string built by the frontend.

## Render

`build_plan` walks tracks in order. Muted tracks are skipped. Within a track, the later start time is composited above the earlier one. Video clips are trimmed, scaled to fit, and padded with transparent black so a portrait clip does not hide the track underneath. Audio clips are trimmed, delayed, gained, and mixed with `amix=normalize=0`. A timeline with no audio clips still writes a silent stereo stream.

Preview settings are half resolution (even), `ultrafast`, CRF 28, AAC 96 kbps. Export defaults are the sequence size, `veryfast`, CRF 18, AAC 160 kbps, H.264 `yuv420p`, `+faststart`. The preview file name is the SHA-256 of the project JSON, the render settings, and each asset's size and modification time, so a new edit is a new URL.

One render runs at a time. Progress is FFmpeg `-progress pipe:1`. Cancel sets an atomic flag checked between progress lines and kills the process. The encoder writes `*.partial.mp4` and the file is renamed only after success.

## IPC

Commands are `engine_report`, `app_info`, `editor_state`, `dispatch_command`, `probe_media`, `new_project`, `open_project`, `save_project`, `start_render`, `cancel_render`, `thumbnail`, `waveform`, `recovery_info`, `restore_recovery`, `discard_recovery`, and `load_sample`.

The asset protocol may read the app cache, the temp directory, and `/tmp`. Source media is not loaded by the webview. Thumbnails and waveforms come back through commands. Preview playback uses `convertFileSrc` on the cache file. `PULSECUT_CACHE` moves recovery files only. Preview files stay in the app cache so the protocol scope still matches.

Capabilities are `core:default` and `dialog:default`. There is no shell plugin. Paths are checked to be existing files before probe or render. FFmpeg is invoked with an argument list, not a shell.

## Recovery

After a successful edit, the shell writes `recovery/autosave.pulsecut` under the cache root. New, open, and save delete it. Restore is a user choice. The restored project is dirty. The playhead is UI state and is not restored.

## What this slice does not do

Effects, transitions, titles, keyframes, scopes, captions, proxies, nesting, interchange, and a GPU compositor are not in the model. Adding them means a schema version bump and a migration, not a parallel timeline.
