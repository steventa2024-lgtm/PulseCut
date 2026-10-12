# PulseCut architecture

This describes what exists in the repository today (v0.1.0, MVP vertical slice). Planned work is marked as such.

## Components

```
apps/desktop/                 Tauri 2 shell
  src/                        React + TypeScript UI (Zustand store, no business logic)
  src-tauri/src/commands.rs   Typed IPC commands: validation, session, jobs
crates/project-model/         Canonical model, edit commands, undo history, .pulsecut I/O
crates/media-engine/          FFmpeg backend: probe, render plan, stills, playback, export, verify
fixtures/media/               Synthetic, redistributable test media
```

```
 ┌─────────────── WebView (React) ───────────────┐
 │ TopBar · MediaBrowser · Viewer · Inspector ·  │
 │ Timeline       (UI state only: zoom, drag,    │
 │                 selection, panel sizes)       │
 └──────┬──────────────────────────▲─────────────┘
        │ invoke(EditCommand, …)    │ ProjectState (full model + history)
        │                           │ raw JPEG frames / PCM (ipc::Channel, ipc::Response)
 ┌──────▼──────────────────────────┴─────────────┐
 │ src-tauri commands.rs                          │
 │  Session { Editor, path, offline set }         │
 │  validation of paths/args · autosave · jobs    │
 └──────┬───────────────────────┬─────────────────┘
        │                       │
 ┌──────▼─────────┐     ┌───────▼───────────────────────────────┐
 │ project-model  │     │ media-engine                          │
 │ Project, Edit- │────▶│ probe() · compile(Project, range) →   │
 │ Command, Editor│     │ RenderPlan · render_still · Playback- │
 │ io::save/load  │     │ Stream · render_audio_pcm · export ·  │
 └────────────────┘     │ verify_export                         │
                        └───────┬───────────────────────────────┘
                                │ child processes (never shell)
                          ffmpeg / ffprobe
```

### Separation of concerns

| Layer | Owns | Must not |
|---|---|---|
| UI (React) | Ephemeral interaction state: drag ghosts, selection, zoom, panel sizes, preview quality | Mutate the project; compute render output |
| Commands (Tauri) | The single `Session`, argument/path validation, background jobs, autosave | Contain edit semantics |
| project-model | Edit semantics, invariants, undo/redo, serialization, migrations | Touch media or processes |
| media-engine | Translating a `Project` into FFmpeg work, running and verifying it | Mutate the project |

**One authoritative timeline model.** The UI receives the full `Project` after every command and renders it; preview stills, playback and export all call `media_engine::compile()` on that same value.

## Project model

- **Time.** `Rational` (normalized `i64/i64`). Timeline positions are integer **frames in the sequence rate**; source in-points are **exact rational seconds**. Converting source in-points never rounds, so mixed-rate clips (25 fps in a 30 fps sequence) do not drift. Durations of mixed-rate sources are floored to whole sequence frames with a 1/1000-frame tolerance.
- **Clip** = `{ id, assetId, start (frames), duration (frames), sourceIn (seconds), link?, params }`. Non-destructive: media files are never modified.
- **Track** order: video tracks composite bottom-to-top in vector order; audio tracks are mixed. Track flags: `muted` (video: hidden from output), `solo` (audio only), `locked`.
- **Linked clips** share a `link` UUID (video + its audio). Move/trim/split/delete apply to the whole group.
- **Stable IDs** are UUIDv4 for project, sequence, tracks, clips and assets.
- **Overwrite semantics.** Adding or moving a clip overwrites whatever it lands on (trim, remove, or split the underlying clip). Trims clamp to source media limits and to the neighbouring clip.

### Edits and undo

`EditCommand` (tagged JSON enum) is the only way to change the timeline: `addClip, moveClip, trimClip, split, deleteClips (ripple), setClipParams, setTrackState, addTrack`. Import/relink/remove-asset go through `Editor::mutate`.

Each command runs on a clone of the project and is committed only if it succeeds **and** the result passes `validate_project` (no overlaps, valid ranges, asset references, kind compatibility, parameter bounds). Undo stores the full previous snapshot (bounded to 500 steps), so undo restores the exact prior model. This is verified by a property test of 300 random 1–40 step edit sequences.

### File format (`.pulsecut`)

UTF-8 JSON, deterministic field order, `"format": "pulsecut.project"`, `"schemaVersion": 1`.

- Saved atomically: temp file in the same directory → `fsync` → rename.
- Each asset stores its absolute path and a path relative to the project file. On open, the absolute path is tried first, then the relative path (so a moved project folder relinks automatically); anything else is reported **offline** and can be relinked in the UI.
- `from_json` runs `migrate(value, version)`: newer schemas are refused with an actionable message, older ones will get explicit `vN → vN+1` steps. Files above 64 MiB are refused.
- Autosave: after every edit the session is written to `<app local data>/autosave/recovery.pulsecut`; it is deleted on save/open/new/discard and offered for recovery on the next launch.

## Media engine

### Probe

`ffprobe -show_format -show_streams -print_format json` → `MediaInfo`: kind (video/audio/image), container, exact duration, size, first video stream (codec, size, `avg_frame_rate` as rational, VFR flag when `r_frame_rate` ≠ `avg_frame_rate`, rotation, HDR transfer flag), first audio stream (codec, rate, channels) and audio stream count. Corrupt or empty files produce `MediaError::Probe` with a user-presentable message.

### Render plan (the parity mechanism)

`compile(project, RenderRequest { start_frame, end_frame, scale, video, audio, offline })` returns FFmpeg input arguments plus one `-filter_complex` graph:

- Video: black `color` base at output size → for each visible clip in track order: `-ss (in − preroll) -t (len + preroll) -i file` → `setpts=PTS+(offset−preroll)/TB, fps=SEQ, trim=start:end, scale (fit × clip scale), format=yuva420p, colorchannelmixer (opacity)` → `overlay x:y enable=between(t,…)`.
- Audio: `anullsrc` base of exact duration → per clip `aresample, aformat=fltp:stereo, asetpts, atrim, volume=dB, adelay=<samples>S` → `amix normalize=0 duration=first → atrim`.

The same function is used for:

| Use | Range | Scale | Output |
|---|---|---|---|
| Paused preview | 1 frame | 0.25 / 0.5 / 1.0 | JPEG via `ipc::Response` |
| Playback | playhead → end | preview scale | MJPEG stream via `ipc::Channel`, paced to wall clock (≤ 250 ms lead) |
| Playback audio | 20 s chunks | — | s16le PCM via `ipc::Response` |
| Export | whole sequence | 1.0 or 0.5 | H.264/AAC MP4 or MOV |

### Playback synchronisation

The frontend starts the frame stream and the first audio chunk, waits for the first frame (≤ 4 s), then starts the Web Audio source. **The audio clock is the master**: every display refresh shows the newest frame whose index is ≤ `start + floor(audioTime × fps)` and counts skipped frames as drops (shown in the viewer). If the audio context cannot run, a `performance.now()` clock is used.

### Export

`export()` refuses empty timelines and offline media, picks `libx264` (else `libopenh264`, else `h264_mf`), writes to a hidden sibling `.<name>.partial.mp4`, parses `-progress pipe:1` (`out_time_us`) for real progress, supports cancellation (kill + delete partial), and renames on success. `verify_export()` then re-probes and fully decodes the file and checks frame size, frame rate, codec, duration (±1 frame + 1 AAC frame), sample rate/channels and decode errors. The UI shows the check table.

## Preview/export invariants

1. A frame rendered by `render_still(n)` and frame `n` decoded from an export of the same project differ only by encoder loss (asserted: PSNR > 35 dB at clip boundaries and interior frames; measured 41–77 dB).
2. Exported duration = `sequence.duration_frames / fps` within one frame plus one AAC frame.
3. Exported frame size and rate equal the sequence (or exactly half size).
4. Audio placement is sample-accurate: a clip at frame `f` starts sounding at `f / fps` seconds (asserted within one frame; measured 21 µs).
5. Muted/hidden tracks and offline clips are excluded identically in preview and export (export refuses offline media).
6. A failed or cancelled export leaves no file at the destination.

## Acceptance tests (mapping)

| # | Requirement | Where | Status |
|---|---|---|---|
| 1 | MP4 import shows duration, size, rate, audio | `end_to_end::probe_reports_real_metadata`; GUI metadata panel | automated |
| 2 | Two clips + music play with in/out and sync | `end_to_end::full_editing_session…` (audio onset, frame content) + GUI playback | automated + manual |
| 3 | Split/trim/move change preview and export consistently | same test: PSNR parity after split/trim/move/PiP | automated |
| 4 | Save → reopen preserves timeline | `edits::serialization_roundtrip…`, e2e save/load equality | automated |
| 5 | Undo/redo restores exact model; failed edits don't corrupt | property test + `locked_tracks_reject…` | automated |
| 6 | Export valid: size, duration, rate, audio, decodable | `verify_export` in e2e and in the app after every export | automated |
| 7 | Export frames match preview | e2e PSNR table | automated |
| 8 | Missing/corrupt/unsupported media → explicit errors | `corrupt_and_missing…`, `offline_media_blocks_export…`, relink UI | automated + manual |
| 9 | Performance on Windows / RTX 3070 Ti class | — | **not run** (no Windows or GPU machine available) |
| 10 | All visible actions work; keyboard; resizing | manual GUI pass (see PR) | manual |

## Security model

- Tauri capabilities: `core:default`, window destroy/set-title, dialog open/save/ask/message. **No fs, shell, http or opener plugins.** CSP restricts scripts to the bundle.
- The frontend never supplies FFmpeg arguments. Commands accept typed values (UUIDs, frame numbers, enums, bounded numbers) and user-picked paths, which are validated (absolute, NUL-free, extension allow-list, file exists / parent dir exists).
- FFmpeg/ffprobe are started directly with an argument vector (no shell), with `CREATE_NO_WINDOW` on Windows and stdin closed.
- Bounded resources: stderr tails capped at 16 KiB, playback lead ≤ 250 ms, frontend frame queue ≤ 90, audio chunks ≤ 60 s per request, import ≤ 500 files per call, project files ≤ 64 MiB, history ≤ 500 steps.
- No network access, telemetry, accounts or cloud calls.
- Known gaps: project files may reference any local path (they are only probed/decoded); temp files for export live next to the destination, not in a managed temp dir; there is no sandboxing of FFmpeg itself.

## Not yet implemented (architecture-level)

Proxy/cache subsystem, background job queue with multiple exports, multiple sequences / nesting, keyframes, effects/transitions model, plugin interface, interchange (OTIO/EDL/XML), GPU compositor. See [ROADMAP.md](ROADMAP.md).
