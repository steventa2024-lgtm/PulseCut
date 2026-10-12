# PulseCut by ZeroPulse

PulseCut is a free, open-source (GPL-3.0-or-later), local-first, non-linear video editor for the desktop. It is built with Tauri 2, React, TypeScript, Tailwind CSS, and Rust, and it uses FFmpeg as its render engine.

**Status: v0.1.0, an early MVP vertical slice.** It is not a replacement for a professional editor yet. The list below covers only what works today. Everything else is tracked honestly in [`docs/FEATURE_MATRIX.md`](docs/FEATURE_MATRIX.md).

## What works today

- Import video, audio, and still images. FFmpeg probes their metadata: duration, resolution, frame rate, codecs, audio layout, and notices for VFR, rotation, and HDR.
- A multitrack timeline that starts with V1, V2, A1, and A2 and lets you add more tracks. Video clips bring linked audio with them.
- Editing operations:
  - drag from the media browser, or add at the playhead
  - move, trim, split (razor), delete, and ripple delete
  - snapping, zoom, and fit
  - track lock, mute, solo, and hide
- Clip properties in the inspector: position, scale, opacity, and gain.
- Undo and redo of every edit, with 500 exact snapshot steps.
- Preview rendered by the same compiled FFmpeg graph that export uses. Preview and export agree with each other (PSNR above 35 dB, checked in automated tests at sampled frames).
- Synced playback with audio as the master clock and a counter for dropped frames. Also seek, frame stepping, and timecode.
- Thumbnails and waveforms on clips.
- Projects:
  - `.pulsecut` project files, saved atomically and versioned by schema
  - autosave with a crash-recovery prompt
  - offline-media detection and relinking
- Export to H.264/AAC MP4 or MOV:
  - real progress reporting and cancel; a cancelled export leaves no partial file
  - automatic checks on the output afterwards (probe, duration, frame rate, codec, audio, and a full decode)

The Rust test suite runs this whole path end to end against real media: import, edit, undo/redo, save/reopen, export, verify, and compare preview with export.

## Known limitations

- **Windows 11 is the primary target, but this build has only been compiled and run on Linux** (Ubuntu 24.04). There is a Windows CI job; until it passes and someone verifies the build by hand, Windows support should be treated as unverified.
- FFmpeg is **not bundled**. It must be installed separately (see below).
- Preview is rendered on the CPU by FFmpeg. There is no GPU path and no proxies yet, and nobody has benchmarked performance on real hardware.
- Not built yet: transitions, titles, keyframes, effects, color tools, captions, and interchange formats. See the [roadmap](docs/ROADMAP.md).

## Requirements

| | Windows 11 x64 | Linux (Ubuntu 24.04) |
|---|---|---|
| Rust | 1.99.0 (pinned in `rust-toolchain.toml`, installed by rustup) | same |
| Node.js | 22 LTS + npm | same |
| FFmpeg | 6.x or newer with `ffmpeg` and `ffprobe` on PATH, or set `PULSECUT_FFMPEG_DIR` | 6.x or newer |
| Webview | WebView2 (preinstalled on Windows 11) | WebKitGTK 4.1 |
| Build tools | Visual Studio 2022 Build Tools, "Desktop development with C++" | `build-essential` |

### Windows setup

```powershell
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
winget install Gyan.FFmpeg
winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive"
```

### Linux setup (Ubuntu/Debian)

```bash
sudo apt install build-essential curl ffmpeg libwebkit2gtk-4.1-dev libgtk-3-dev \
  libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build and run

```bash
cd apps/desktop
npm ci
npm run tauri dev                         # development, with hot reload
npm run tauri build -- --debug --no-bundle  # debug binary at target/debug/pulsecut-desktop
npm run tauri build                       # release installers (NSIS/MSI on Windows, deb/AppImage on Linux)
```

PulseCut looks for FFmpeg in these places, in order:

1. `PULSECUT_FFMPEG_DIR`
2. the executable's directory
3. an `ffmpeg/` folder next to the executable
4. `PATH`

The Help → Engine status dialog shows which binaries and encoders were found.

## Tests

```bash
./scripts/generate-fixtures.sh   # regenerates the original synthetic test media (already committed)
./scripts/test-all.sh            # cargo test + clippy + frontend typecheck, unit tests, build
```

On Windows, `scripts\build-windows.ps1` runs the same steps. It is written for Windows but has **not been run on Windows yet**.

## Supported formats

PulseCut can import anything the installed FFmpeg can decode, as long as the file extension is on the allow-list in `apps/desktop/src-tauri/src/commands.rs`:

- video: mp4, mov, m4v, mkv, webm, avi, mxf, mts, m2ts
- audio: wav, mp3, aac, m4a, flac, ogg, opus
- images: png, jpg, jpeg, webp, bmp

Export currently produces H.264 with AAC audio, in an MP4 or MOV container.

## Repository layout

```
crates/project-model   canonical timeline model, edit commands, undo/redo, .pulsecut I/O
crates/media-engine    FFmpeg discovery, probe, graph compiler, preview/playback/export, verification
apps/desktop           Tauri 2 shell (src-tauri) and React UI (src)
docs/                  architecture, engine evaluation, design system, feature matrix, roadmap, ADRs
fixtures/media         original, synthetic, license-safe test media
scripts/               fixture generation, engine spike, test and build scripts
```

## Documentation

- [Architecture](docs/ARCHITECTURE.md), including the invariants that keep preview and export in agreement
- [Engine evaluation](docs/ENGINE_EVALUATION.md): FFmpeg compared with MLT, with measurements
- [Design system](docs/DESIGN_SYSTEM.md)
- [Feature matrix](docs/FEATURE_MATRIX.md) and [Roadmap](docs/ROADMAP.md)
- [License audit](docs/LICENSE_AUDIT.md) and [ADRs](docs/DECISIONS/)

## License

PulseCut is licensed under the GNU General Public License v3.0 or later (see [`LICENSE`](LICENSE)). FFmpeg is a separate program that PulseCut runs as a child process. FFmpeg's own license depends on how it was built, and a GPL build is fine to use alongside PulseCut. See [`docs/LICENSE_AUDIT.md`](docs/LICENSE_AUDIT.md).

Contributions are welcome; see [`CONTRIBUTING.md`](CONTRIBUTING.md).
