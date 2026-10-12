# Media engine evaluation

Decision: the first PulseCut renderer is the FFmpeg executable, invoked as a separate process. MLT was measured and not adopted. Neither library is linked into the Rust crates. FFmpeg is not vendored.

## Host

These measurements were taken on the cloud VM that produced this repository, not on a Windows PC and not on an RTX 3070 Ti.

- Linux 6.12, Ubuntu 24.04.5, KVM, 4 vCPU, about 15 GiB RAM, no GPU (`nvidia-smi` absent)
- FFmpeg 6.1.1-3ubuntu5, configured with `--enable-gpl`, libx264, libx265, native AAC, libvpx, libaom
- melt 7.22.0 from Ubuntu packages

## What had to be proven

Import MOV and MP4, decode a frame, seek, show timecode, composite two video tracks, mix audio, write a file, reopen it, and compare a few preview frames with the export.

## FFmpeg spike

Sources were synthetic: a 1280×720 30 fps red clip (5 s, 440 Hz), a 1920×1080 24 fps blue clip (4 s, 880 Hz, 44.1 kHz), and a stereo music bed. The graph was a black base, trim, `setpts`, scale, pad, fps, `overlay=eof_action=pass:repeatlast=0`, and `amix` with `normalize=0`.

| Output | Wall time | Result |
| --- | --- | --- |
| 1280×720 30 fps, veryfast, CRF 18, AAC 160 kbps | 0.685 s | 6.000000 s, H.264 + AAC 48 kHz stereo, 136490 bytes |
| 640×360 ultrafast CRF 30 proxy | 0.419 s | 84317 bytes |

Sampled colors after yuv420: t=0.5 and t=1.0 near red `(225,58,58)` from `0xE23B3B`; t=3.0 and t=4.5 near blue `(45,109,236)` from `0x2F6FED`; t=5.5 black. PSNR of the proxy against the export, both scaled to 640×360: t=1.0 average 49.88 dB, t=3.0 52.86 dB, t=5.5 77.67 dB. Fast seek of one frame was 0.094 s. Accurate seek was 0.128 s.

Two FFmpeg behaviors matter:

- On this build, both `out_time_us` and `out_time_ms` in `-progress` are microseconds. Progress uses `out_time_us`.
- `amix` defaults to `normalize=1`. The graph sets `normalize=0`.
- `pad=color=black` is opaque and hides lower tracks. The shipped graph uses `black@0`, `format=rgba`, and `overlay=format=auto`. A later integration test checks that a portrait green clip leaves the lower red corners visible.

## MLT spike

`melt` 7.22.0 produced a 1280×720 30 fps H.264 AAC file in 0.923 s. Colors at 1.0 s and 3.0 s matched the intended stack. The file duration was 5.034 s against a requested 6 s. The sample at 5.5 s could not be read. That duration miss, plus an unproven Windows packaging story for MLT modules, is why MLT is not the engine. The `melt` binary on this VM is GPL-2.1-or-later for the framework and GPL-2-or-later for the melt CLI. Some modules, including `plusgpl`, have their own terms. They are not dependencies of this tree.

## Regression that replaces the one-off spike

`cargo test -p media-engine --test pipeline` builds its own synthetic MOV, MP4, and M4A files and checks:

- probe width, height, frame rate, and audio presence
- a two-clip plus music timeline saved and reloaded at 6 s
- export H.264/AAC, 1280×720, duration within 50 ms, stereo 48 kHz
- red, blue, red, then black frames at 1.0, 3.0, 4.5, and 5.5 s
- audible audio after the picture ends
- preview-versus-export PSNR above 30 dB at 1.0 s and 3.0 s
- a trim that changes the exported frame
- a JPEG thumbnail, 32 waveform peaks, a corrupt file that fails probe
- a video-only timeline that still writes a silent audio stream
- letterbox compositing that does not cover the lower track

## Failure modes

| Condition | Behavior |
| --- | --- |
| `ffmpeg` or `ffprobe` missing | `engine_report.ok` is false. Play and export explain that. Override with `PULSECUT_FFMPEG` and `PULSECUT_FFPROBE`. |
| No libx264 or no AAC encoder | Render refuses before starting. |
| Missing or offline media | Render returns `MissingMedia` and the UI shows the message. |
| Empty or fully muted timeline | Render returns an error instead of an empty success. |
| Cancel | The partial file is deleted and the UI receives `render-cancelled`. |
| Unsupported size or preset | Even dimensions from 160×160 through 7680×4320. x264 presets are a whitelist. CRF is 0–51. |

Hardware decode and encode were not measured. This VM has no GPU. Windows MLT and Windows FFmpeg packaging were not built here.

## Why not libav in-process

Linking libav would pull its license into the binary and make a missing codec a compile-time choice. A subprocess keeps the system or user-supplied FFmpeg outside the PulseCut binary, which matches the decision not to ship codec libraries from this repository. The cost is process startup and no frame-accurate live scrub yet. Scrubbing seeks inside the latest proxy file.
