# ADR 0004 — Preview through the export render plan

- Status: accepted (2026-10-12)

## Decision

- **Paused preview** renders the exact composited frame with the export render plan (JPEG at ¼, ½ or full resolution; full = export resolution).
- **Playback** streams MJPEG frames from one FFmpeg process running the same plan from the playhead, paced in Rust to at most 250 ms ahead of wall-clock, delivered over a Tauri `ipc::Channel` as raw bytes. Audio is rendered by the same plan to PCM in 20 s chunks and played with Web Audio. The audio clock is master; late frames are dropped and counted.
- HTML `<video>` elements on source files are **not** used for preview.

## Why

`<video>` playback in the WebView would use different decoders (WebView2/GStreamer), different scaling and cannot composite our tracks identically, breaking the preview/export parity requirement. Using the render plan keeps one interpretation of the timeline.

## Consequences

- Playback cost is a full software decode + composite + MJPEG encode/decode; at high resolutions the ½/¼ preview qualities are needed. No measurements on Windows or 4K yet.
- Startup latency is one FFmpeg launch plus the first audio chunk render; it has not been benchmarked yet.
- Edits during playback stop playback (the stream reflects the project at start time).
