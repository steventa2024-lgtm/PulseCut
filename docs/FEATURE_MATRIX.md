# Feature matrix and implementation ledger

Status values: **Not started · Prototype · Implemented · Tested · Released**.
"Tested" means covered by an automated test in this repo *and* exercised manually in the app. Nothing is **Released** yet (no public binary). Difficulty: S / M / L / XL.

> As of v0.1.0, "Tested" rows pass their automated tests on Linux. The manual GUI pass (import → edit → playback → save/reopen → export from the UI) on the Linux development build is still in progress, and nothing has been run on Windows.

Comparisons with other editors refer to publicly documented, user-visible workflows only; no claim of parity is made for anything not listed as Tested.

## A. Editorial / timeline

| Feature | Status | Depends on | Diff. | Tests | Milestone |
|---|---|---|---|---|---|
| Import video/audio/stills via FFmpeg probe | Tested | ffprobe | S | `probe_reports_real_metadata`, GUI | MVP |
| Media metadata panel (duration, size, fps, audio, VFR/HDR/rotation notices) | Implemented | probe | S | GUI | MVP |
| Media search (by name) | Implemented | — | S | GUI | MVP |
| Bins, tags, metadata search | Not started | asset model v2 | M | — | Phase 2 |
| Offline media detection + relink (absolute → relative → manual) | Tested | io::load | M | `serialization_roundtrip_and_relative_relink`, `offline_media_blocks_export…` | MVP |
| Proxies / proxy toggle | Not started | job queue, cache | L | — | Phase 2 |
| Multitrack timeline (N video, N audio tracks) | Tested | model | M | `edits.rs`, GUI | MVP |
| Add clip (overwrite) by drag & drop / at playhead | Tested | model | M | `overwrite_splits_underlying_clip`, GUI | MVP |
| Insert edit (shift downstream) | Not started | model | M | — | Phase 2 |
| Move clip (within / across same-kind tracks), overwrite on drop | Tested | model | M | property test, e2e | MVP |
| Trim in/out with media + neighbour clamping | Tested | model | M | `end_trim_clamps…`, e2e | MVP |
| Split / razor at playhead (all or selected) | Tested | model | S | e2e, GUI | MVP |
| Delete / ripple delete (per track) | Tested | model | S | `ripple_delete_closes_gap` | MVP |
| Rolling, slip, slide edits | Not started | model | M | — | Phase 2 |
| Snapping (clip edges, playhead, 0) with indicator | Implemented | UI | S | `time.test.ts` (snap logic), GUI | MVP |
| Timeline zoom (buttons, slider, Ctrl+wheel at cursor, fit) | Implemented | UI | S | GUI | MVP |
| Markers, in/out ranges | Not started | model | S | — | Phase 2 |
| Track lock / mute / solo (audio) / hide (video) | Tested | model + plan | S | e2e (muted track), `locked_tracks_reject…` | MVP |
| Track height resize | Implemented | UI | S | GUI | MVP |
| Linked audio/video (edit together) | Tested | model | M | `add_video_creates_linked_audio…`, split test | MVP |
| Unlink / relink A/V, grouping | Not started | model | S | — | Phase 2 |
| Multi-select (selection only; edits apply per clip) | Prototype | UI | M | GUI | Phase 2 |
| Nesting / compound clips, multiple sequences | Not started | model v2 | L | — | Phase 2 |
| Adjustment layers | Not started | effects model | M | — | Phase 2 |
| Sequence presets (1080p/720p/4K/vertical/square, 23.976–60 fps) | Implemented | model | S | GUI | MVP |
| Mixed frame rates (exact rational source time) | Tested | time.rs, plan | M | `mixed_rate_source_length_is_exact`, e2e parity | MVP |
| Variable-frame-rate sources (flagged, resampled with `fps`) | Implemented | plan | M | — (no VFR fixture yet) | MVP |
| Multicam | Not started | sync, model v2 | XL | — | Phase 3 |
| Speed / reverse / freeze / time remap | Not started | plan, model | L | — | Phase 3 |
| Stabilization (vid.stab) | Not started | job queue | M | — | Phase 3 |
| Scene detection, auto-reframe | Not started | analysis | L | — | Phase 4 |
| Undo/redo (exact snapshots, 500 steps) | Tested | history | S | property test, e2e | MVP |
| Atomic save, schema versioning, newer-schema refusal | Tested | io | S | `rejects_newer_schema_and_garbage` | MVP |
| Autosave + crash recovery prompt | Implemented | commands.rs | S | GUI | MVP |

## B. Video / compositing / motion

| Feature | Status | Depends on | Diff. | Tests | Milestone |
|---|---|---|---|---|---|
| Transform: scale, position | Tested | plan | S | e2e PiP parity | MVP |
| Opacity | Tested | plan | S | e2e PiP parity | MVP |
| Crop, rotation, anchor | Not started | plan | S | — | Phase 2 |
| Keyframes / easing | Not started | model v2 | L | — | Phase 2 |
| Transitions (cross-dissolve, dip, wipes) | Not started | model, plan (`xfade`) | M | — | Phase 2 |
| Basic effects (blur, sharpen, denoise) | Not started | effects model | M | — | Phase 2 |
| Titles / text | Not started | text renderer (`drawtext`/libass) | M | — | Phase 2 |
| Image overlays | Implemented (stills on video tracks) | plan | S | — | MVP |
| Color correction, LUTs | Not started | plan (`lut3d`) | M | — | Phase 3 |
| Scopes (waveform, vectorscope, histogram) | Not started | frame analysis | M | — | Phase 3 |
| Masks, chroma key, track matte, blend modes | Not started | compositor | L | — | Phase 3 |
| Motion tracking | Not started | analysis | XL | — | Phase 3 |
| Node compositor workspace | Not started | compositor | XL | — | Phase 4 |
| Motion graphics templates, shape layers | Not started | compositor | XL | — | Phase 4 |
| GPU compositing path | Not started | engine v2 | XL | — | Phase 3 |

## C. Audio

| Feature | Status | Depends on | Diff. | Tests | Milestone |
|---|---|---|---|---|---|
| Multitrack mixing, sample-accurate placement | Tested | plan | M | e2e audio onset, `audio_pcm_has_exact_length` | MVP |
| Clip gain (dB) | Implemented | plan | S | GUI | MVP |
| Waveforms on clips | Implemented | analysis | S | GUI | MVP |
| Sample-rate conversion (44.1 → 48 kHz) | Tested | plan (`aresample`) | S | e2e (clip_b is 44.1 kHz) | MVP |
| Fades / crossfades | Not started | model | S | — | Phase 2 |
| Pan, mixer, buses | Not started | model | M | — | Phase 2 |
| Audio keyframes | Not started | keyframes | M | — | Phase 2 |
| EQ, compressor, limiter, noise reduction | Not started | effects model | M | — | Phase 3 |
| Loudness metering / normalization (EBU R128) | Not started | analysis (`ebur128`) | M | — | Phase 3 |
| VST plugins | Not started | sandboxing research | XL | — | Phase 4+ |

## D. Captions / AI / productivity

| Feature | Status | Depends on | Diff. | Tests | Milestone |
|---|---|---|---|---|---|
| Manual captions, SRT/VTT import/export | Not started | model | M | — | Phase 3 |
| Burned-in captions | Not started | libass in plan | M | — | Phase 3 |
| Offline transcription (whisper.cpp, opt-in model download) | Not started | job queue | L | — | Phase 4 |
| Text-based editing, silence removal | Not started | transcription | L | — | Phase 4 |
| Background removal, subject tracking | Not started | ML runtime | XL | — | Phase 4 |

## E. Output / interchange / performance

| Feature | Status | Depends on | Diff. | Tests | Milestone |
|---|---|---|---|---|---|
| Export MP4/MOV H.264 + AAC (libx264 → openh264 → MF fallback) | Tested | export | M | e2e, GUI | MVP |
| Quality presets (High/Standard/Draft), full/half resolution | Implemented | export | S | GUI | MVP |
| Real progress, cancel, atomic output | Tested | export | S | `export_cancel_leaves_no_partial_file` | MVP |
| Post-export verification (probe + full decode) | Tested | verify | S | e2e, GUI | MVP |
| H.265 / AV1 / VP9 / WebM / ProRes | Not started | export presets | S each | — | Phase 2 |
| WAV/MP3/AAC audio-only export | Not started | export | S | — | Phase 2 |
| Image sequence, alpha, GIF | Not started | export | M | — | Phase 3 |
| Hardware encode (NVENC/QSV/AMF/MF) | Not started (MF only as fallback) | tools detection | M | — | Phase 3 |
| Batch export queue, pause | Not started | job queue | M | — | Phase 2 |
| OTIO / EDL / FCPXML / AAF interchange | Not started | model mapping | L | — | Phase 3 |
| Thumbnail + waveform cache (in memory per session) | Implemented | analysis | S | — | MVP |
| Persistent cache with size limits / location | Not started | cache service | M | — | Phase 2 |
| Real-time playback (engine-rendered, audio-clock sync, drop counter) | Implemented | playback | L | `playback_stream_delivers_ordered_frames`, GUI | MVP |
| Performance benchmarks on Windows / RTX 3070 Ti class | Not started | Windows CI / hardware | M | — | Phase 1 follow-up |

## Platform

| Platform | Status |
|---|---|
| Linux x86_64 (Ubuntu 24.04, WebKitGTK 2.52) | Implemented — built and run in development VM |
| Windows 11 x64 (WebView2) | **Not verified** — CI job defined, results pending |
| macOS | Not started |
