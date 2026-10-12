# PulseCut

PulseCut is a free, local, open-source video editor from ZeroPulse. This repository is the first editing slice: a project model, an FFmpeg render path, and a dark desktop shell. It is not a finished professional suite, and it does not claim parity with any other editor.

License: [GPL-3.0-or-later](LICENSE).

## What works

The automated tests on Linux cover this path:

1. Probe an MP4 or MOV for duration, frame size, frame rate, and audio.
2. Place two video clips and a music clip on a timeline.
3. Render a real H.264/AAC MP4.
4. Check frame color at several times, and check that audio continues after the picture ends.
5. Trim a clip and render again so the export changes with the edit.
6. Round-trip the project JSON and undo back to the same document.

The desktop window can import media, show the bin, drag clips to tracks, play a rendered proxy, step frames, trim, split, move, delete, undo, save, reopen, and export. On this VM the window pass loaded the sample timeline, played the proxy, saved a `.pulsecut` file, reopened it after a delete, trimmed a clip, undid the trim, and exported a 6.000 s 1280×720 H.264/AAC MP4. Drag-to-timeline, bin search, relink, and unlink were not clicked. The app is not marked MVP-complete. See [docs/FEATURE_MATRIX.md](docs/FEATURE_MATRIX.md).

## What does not work yet

Transitions, effects, titles, keyframes, color grading, captions, multicam, interchange, hardware encode, and a Windows installer are not implemented. The Effects menu only opens that status. There is no account, subscription, watermark, or network service.

Preview is a half-resolution H.264 proxy, not a live GPU compositor. The first play after an edit waits for FFmpeg.

## Requirements

- Rust 1.99.0 (`rust-toolchain.toml`)
- Node.js 22 and pnpm 10
- FFmpeg and FFprobe on `PATH`, with libx264 and an AAC encoder
- For the desktop window: Tauri 2's WebView (WebView2 on Windows, WebKitGTK on Linux). Linux playback also needs a GStreamer H.264 decoder, for example `gstreamer1.0-libav`, plus the base and good plugin packages.

FFmpeg is not bundled. Set `PULSECUT_FFMPEG` and `PULSECUT_FFPROBE` if the binaries are not on `PATH`.

Checked on this tree's development VM: Ubuntu 24.04, FFmpeg 6.1.1, no GPU. Windows 11 is the intended first desktop target and has not been built here. `scripts/windows-dev.ps1` is an unrun checklist.

## Build and run

```bash
cargo test --workspace
bash scripts/generate_fixtures.sh
cd apps/desktop
pnpm install
pnpm test
pnpm tauri dev
```

In the app, File → Load sample timeline uses `fixtures/media`. Play renders the proxy. Export MP4 writes a file you choose and shows the probed result.

Frontend production bundle only:

```bash
cd apps/desktop && pnpm exec vite build
```

## Project layout

```
apps/desktop/          Tauri shell and React UI
crates/project-model/  Timeline, edits, undo, .pulsecut JSON
crates/media-engine/   FFmpeg probe, graph, preview, export
fixtures/              Synthetic media
docs/                  Architecture, design, engine, license, roadmap
scripts/               Fixture generation and Windows notes
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Design system](docs/DESIGN_SYSTEM.md)
- [Engine evaluation](docs/ENGINE_EVALUATION.md)
- [Feature matrix](docs/FEATURE_MATRIX.md)
- [Roadmap](docs/ROADMAP.md)
- [License audit](docs/LICENSE_AUDIT.md)
- [Decisions](docs/DECISIONS/)
- [Contributing](CONTRIBUTING.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

## Known limitations

- Timecode display is non-drop. 29.97 drop-frame counting is not implemented.
- Solo, ripple, slip, slide, and roll edits are absent.
- Variable-frame-rate and HDR files are flagged, not conformed.
- One render at a time. Cancel stops the FFmpeg process.
- File dialogs need the platform dialog backend. On the development VM the desktop portal package was left half-configured, so dialogs were not verified.
- Performance numbers in the engine doc are from a 4 vCPU VM without a GPU. They are not measurements from an RTX 3070 Ti.
