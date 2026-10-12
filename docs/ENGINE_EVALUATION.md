# Media engine evaluation

Status: **decision made for the MVP** (see [ADR 0001](DECISIONS/0001-ffmpeg-filter-graph-engine.md)). Re-evaluate before Phase 2 transitions/effects work.

## Question

Which engine should PulseCut use for timeline composition, multitrack playback, audio mixing and export, with preview that matches export?

Candidates evaluated:

| Option | Description |
|---|---|
| **A. MLT Framework 7 + FFmpeg** | MLT XML (tractor / playlists / transitions) rendered by `melt` or `libmlt` with the `avformat` producer/consumer. Used by Shotcut and Kdenlive. |
| **B. FFmpeg filter graph (PulseCut render plan)** | PulseCut compiles its own project model into one `ffmpeg -filter_complex` graph per render request. FFmpeg runs as a child process. |
| C. GStreamer Editing Services (GES) | Considered on paper only (see “Not tested”). |
| D. Custom decoder + GPU compositor (e.g. `ffmpeg-next` + wgpu) | Long-term option for real-time effects; far too large for an MVP. |

## Environment of the spike (honest scope)

The spike ran on the machine this repository was developed on, **not on Windows 11**:

| Item | Value |
|---|---|
| OS | Ubuntu 24.04 (Linux 6.12, x86_64), cloud VM |
| CPU / RAM | 4 vCPU, 15 GiB |
| GPU | none (software rendering only) |
| FFmpeg | 6.1.1-3ubuntu5 (distribution build, `--enable-gpl`, libx264/libx265/libsvtav1/libvpx/libaom) |
| MLT | 7.22.0 (`melt`, Ubuntu packages `melt libmlt7 libmlt++7`) |

**Windows has not been tested yet.** The Windows CI job in `.github/workflows/ci.yml` is the first planned Windows evidence; until it passes, Windows support is a target, not a fact.

## Spike timeline

Reproducible from `scripts/spike/` with the synthetic fixtures from `scripts/generate-fixtures.sh`:

- V1: `clip_a` (30 fps MP4) source 1.0–3.0 s at t = 0–2 s, then `clip_b` (25 fps MOV) source 0–2 s at t = 2–4 s
- V2: `clip_b` picture-in-picture (320×180, top-left) at t = 1–2 s
- A1: 6 s WAV music bed (first 4 s used); both clips' own audio mixed in
- Sequence: 640×360, 30 fps, 48 kHz stereo, 4 s

Option A: `scripts/spike/timeline.mlt` → `melt timeline.mlt -consumer avformat:out.mp4 vcodec=libx264 acodec=aac`.
Option B: `scripts/spike/ffmpeg_graph.sh` (hand-written equivalent of what `crates/media-engine/src/plan.rs` now generates).

## Results

| Check | MLT 7.22 | FFmpeg graph |
|---|---|---|
| Imports MP4 + MOV (mixed 30/25 fps) | yes | yes |
| Two overlapping video tracks (PiP) | yes (`affine` transition) | yes (`overlay`) |
| Audio mix of 3 sources | yes (`mix` transition, `sum=1`) | yes (`amix normalize=0`) |
| Output file valid (ffprobe) | h264 640×360 30 fps + AAC 48 kHz, 4.010 s | h264 640×360 30 fps + AAC 48 kHz, **4.000 s**, 120 frames |
| Wall time, 4 s render (incl. process start) | ~0.7 s | 0.29 s |
| Single composited still (preview frame) | 171 ms | 93–169 ms |
| Muxer warnings | "Timestamps are unset in a packet for stream 1" / "Encoder did not produce proper pts" | none |
| Mean / max audio level (same sources) | −22.1 / −10.6 dB | −22.1 / −11.7 dB |
| Exported frame vs other engine (PSNR, frames 45 / 75) | — | 34.0 / 29.8 dB (different scaler and 25→30 fps frame mapping) |

**Preview vs export parity, Option B** (lossless still from the render plan vs the same frame decoded from the exported H.264 file):

| Frame | 0 | 45 | 75 | 119 |
|---|---|---|---|---|
| PSNR (dB) | 47.5 | 39.9 | 53.5 | 48.5 |

The residual difference is H.264 compression at CRF 20. The bottom-right quadrant of frame 45 was **bit-identical** (PSNR ∞) to `clip_a` decoded at source time 2.5 s, confirming the source-relative in-point arithmetic.

The production engine is held to the same bar by `crates/media-engine/tests/end_to_end.rs` (see “Bug found by the parity test” below).

## Decision criteria

| Criterion | MLT + FFmpeg | FFmpeg graph |
|---|---|---|
| Preview/export parity | Good (same graph via different consumers) | Good by construction: one render-plan compiler for stills, playback and export |
| Windows packaging | Heavy: Shotcut ships ~100 DLLs built with MSYS2; no maintained Rust bindings, so PulseCut would need FFI to the C API or drive `melt` | One `ffmpeg.exe` + `ffprobe.exe` (or user-installed FFmpeg) |
| Licensing | MLT core LGPL-2.1; `plusgpl`, `vidstab`, `opencv`, etc. modules GPL; plus FFmpeg's own LGPL/GPL split | FFmpeg only; LGPL or GPL depending on the build |
| Real-time playback | `sdl2` consumer is a separate window; embedding in a WebView needs a frame bridge either way | Needs a frame bridge (implemented: paced MJPEG stream + PCM audio) |
| Effects/transitions breadth | Large existing library (frei0r, movit GPU, luma wipes) | FFmpeg filters (large) but transitions/keyframes must be expressed by PulseCut |
| Testability | Good (XML in, file out) | Very good: the plan compiler is a pure function, unit-testable without media |
| Control over semantics | MLT's own model (playlists, in/out inclusive) must be mapped from ours | Exact: PulseCut's model is the source of truth |

## Decision

Use **Option B (FFmpeg filter graph compiled from the PulseCut project model)** for the MVP, behind the `media-engine` crate boundary so the backend can change without touching the project model or UI.

MLT stays the primary candidate for Phase 2/3 if keyframed effects and transitions become cheaper to express in MLT XML than in FFmpeg graphs, **conditional on a Windows packaging proof** (build, bundle, render the spike timeline, measure).

## Bug found by the parity test (and fixed)

The first engine version rendered the **last frame of a 25 fps clip on a 30 fps sequence as black** in stills, while the export showed the correct frame (PSNR 5.4 dB at frame 164). Cause: seeking FFmpeg straight to the in-point (`-ss t`) discards the source frame whose PTS is before `t` even though it is still on screen at `t`; `setpts=PTS-STARTPTS` also shifted between-frame seeks by up to one frame. The fix decodes from up to 0.5 s before the in-point, keeps FFmpeg's seek-relative timestamps, resamples with `fps`, then cuts exactly with `trim` (quarter-frame guard on both bounds). After the fix all sampled frames match at 41–77 dB and the audio onset is sample-accurate (silence ends at 0.500 s and 2.500 s exactly where clips start).

## Known failure modes and limits of the chosen engine

- **Process start and seek cost per still** (~100–170 ms here). Fast scrubbing is "latest request wins", not every frame. A persistent decoder/frame cache is a Phase 2 item.
- **Every clip in the render range opens its own decoder.** Very long timelines with hundreds of clips will need segmenting the render (e.g. per edit-point segment + concat) before Phase 2.
- **Playback is software-only**: preview frames are MJPEG-encoded and decoded in the WebView. Measured only on the VM above; no GPU or 4K numbers exist yet.
- **No hardware decode/encode yet** (`h264_nvenc`, `h264_qsv`, `h264_mf` are not wired except `h264_mf` as a fallback encoder).
- **HDR** sources are flagged but rendered SDR without tone mapping.
- **FFmpeg version differences**: graph syntax (`amix normalize`, `adelay all=1`, `trim`) requires FFmpeg ≥ 5.0. Version gating is not implemented yet.

## Not tested

- GStreamer Editing Services: not installed or measured. It would solve the frame bridge more natively on Linux but adds a heavy, plugin-licensed runtime on Windows.
- Any Windows build of either engine.
- GPU paths (MLT movit, FFmpeg CUDA/QSV/D3D11 filters).

## Reproduce

```bash
scripts/generate-fixtures.sh
cd scripts/spike
melt timeline.mlt -consumer avformat:/tmp/spike_mlt.mp4 vcodec=libx264 acodec=aac frequency=48000 channels=2
./ffmpeg_graph.sh /tmp/spike_ff.mp4 0 4 -c:v libx264 -preset veryfast -crf 20 -c:a aac
cargo test -p pulsecut-media-engine -- --nocapture   # production engine, prints PSNR table
```
