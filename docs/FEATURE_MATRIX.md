# Feature matrix

Status words mean only this:

| Status | Meaning |
| --- | --- |
| Not started | No code |
| Prototype | Code exists and is not covered by an automated test of that behavior |
| Implemented | The command is wired through the desktop shell. The GUI path was not exercised on a display in this environment |
| Tested | An automated test in this repository passed on the Linux VM described in `docs/ENGINE_EVALUATION.md` |
| Released | A versioned public build includes it. Nothing here is Released |

Difficulty is a relative note for the next change, not a schedule. "Desktop GUI" means the Tauri window path, which compiles and has not been clicked through on this VM.

## A. Editorial

| Feature | Status | Dependency | Tests |
| --- | --- | --- | --- |
| Probe MP4/MOV duration, size, rate, audio | Tested | ffprobe | `pipeline::import_timeline_trim_save_and_export` |
| Import into a bin, search the bin | Implemented | Tauri dialog + probe | Desktop GUI not run |
| Bins beyond one list, tags | Not started | | |
| Relink a missing file | Implemented | `updateAsset` | Desktop GUI not run |
| Offline asset flag | Tested | project model | open path marks missing files; pipeline rejects missing media at render |
| Proxies | Not started | | |
| Place, move, trim, split, delete | Tested | project model | `edit.rs` unit tests and pipeline trim |
| Linked audio/video move, trim, split, delete | Tested | `linkId` | `delete_removes_linked_group_and_schema_roundtrip` |
| Unlink | Implemented | command | unit coverage is the command path; GUI not run |
| Undo / redo exact JSON | Tested | snapshot stack | `place_undo_redo_restores_exact_document` |
| Ripple, roll, slip, slide | Not started | | |
| Snap while dragging | Implemented | UI plus frame snap in Rust | Rust snap is tested; drag gesture is not |
| Markers, ranges, grouping, nesting | Not started | | |
| Track mute and lock | Tested | model + graph | `muted_video_track_is_omitted`, `split_trim_and_locked_track` |
| Solo | Not started | deliberately absent from the UI | |
| Multiple sequences | Not started | one sequence per file | |
| Frame size and 24/25/30/60 presets | Implemented | `setSequence` | dimensions validated in the model |
| Variable frame rate and HDR | Prototype | probe flags | displayed as warnings; no conformance test |
| Multicam, speed, freeze, reverse, retime, stabilize, scene detect, reframing | Not started | | |
| Autosave recovery file | Implemented | desktop cache | not an automated test |
| Drag clips to tracks | Implemented | UI | Desktop GUI not run |

## B. Picture

| Feature | Status | Notes |
| --- | --- | --- |
| Scale-to-fit letterbox composite of video tracks | Tested | `portrait_clip_letterboxes_without_covering_the_lower_track` |
| Opacity, blend, mask, key, transform, crop, effects, transitions | Not started | |
| Color, LUT, scopes, keyframes, curves | Not started | |
| Titles, shapes, templates, motion graphics | Not started | |
| Node compositor | Not started | future workspace, not the default UI |

## C. Audio

| Feature | Status | Notes |
| --- | --- | --- |
| Multitrack mix, delay, 48 kHz stereo | Tested | pipeline audible tail; `normalize=0` |
| Per-clip gain | Tested | `gain_is_clamped_and_rejected_on_video`; graph volume |
| Waveform peaks | Tested | 32 buckets in the pipeline test |
| Fades, EQ, compressor, limiter, noise reduction, mixer bus, VST | Not started | |
| Loudness meter | Not started | |

## D. Captions and AI

| Feature | Status | Notes |
| --- | --- | --- |
| Manual captions, SRT/VTT, transcription, text-based edit, translation | Not started | |
| Local models | Not started | must stay off the export path |

## E. Output

| Feature | Status | Notes |
| --- | --- | --- |
| H.264 + AAC MP4 export | Tested | requires libx264 and an AAC encoder in the user's FFmpeg |
| Preview proxy and PSNR check on solid colors | Tested | threshold 30 dB in the pipeline test |
| Export progress and cancel | Implemented | FFmpeg progress events; GUI not run |
| HEVC, AV1, WebM, WAV, image sequence, alpha, GIF | Not started | not advertised |
| Hardware encode | Not started | |
| XML, EDL, OTIO, AAF | Not started | |
| Windows installer | Not started | this tree was not packaged on Windows |
| Thumbnail JPEG | Tested | pipeline |

## Acceptance list from the brief

| Check | Result on this VM |
| --- | --- |
| 1. Import metadata | Tested in the pipeline probe |
| 2. Two clips and music, sync and in/out | Tested by frame samples and volumedetect |
| 3. Trim changes export | Tested |
| 4. Save and reopen the document | Tested as JSON round-trip. Process restart of the GUI was not run |
| 5. Undo/redo | Tested in `project-model` |
| 6. Decodable export, size, duration, rate, audio | Tested |
| 7. Preview vs export | Tested above 30 dB on synthetic color. Not tested on camera footage or transitions |
| 8. Missing and corrupt media | Corrupt probe tested. Missing file tested in the graph unit test |
| 9. RTX 3070 Ti performance | Not run. This VM has no GPU. See the engine doc for the 4 vCPU numbers |
| 10. Every visible control, keyboard, resize | Partial. Space, Home, and the sample timeline were used in the window. Resize and every menu item were not all exercised |

The product is not marked MVP-complete. The Rust path from media to a trimmed MP4 is tested.

A manual window pass on this same VM, after `gstreamer1.0-libav` was installed, did the following. File → the import dialog opened (GTK). Load sample filled the bin with the three fixture files and the four tracks. Space played the proxy: the monitor showed the red frame and the timecode advanced (one capture was `00:00:04:07`, red `(226, 57, 60)`, with Pause shown). At the end of the six-second timeline the monitor was black, which matches the edit. The recovery banner appeared after a restart. The Effects menu only states that effects are not in this build. Export was not clicked through a save dialog in that pass.
