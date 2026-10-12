# License audit

PulseCut is GPL-3.0-or-later. The text is the GNU license in `LICENSE`, copied from `https://www.gnu.org/licenses/gpl-3.0.txt`. This audit is of the components this tree actually uses. It is not legal advice.

## PulseCut code

Original Rust and TypeScript in this repository is GPL-3.0-or-later. The mark (square, blue bar, light bar) is original and generated with FFmpeg `drawbox`, then `tauri icon`. No third-party product screenshots are stored as assets.

## Direct dependencies

| Component | Version pin | License | How it is used |
| --- | --- | --- | --- |
| Tauri | 2.12.3 | MIT or Apache-2.0 | Desktop shell. Feature `protocol-asset` only |
| tauri-build | 2.7.2 | MIT or Apache-2.0 | Build script |
| tauri-plugin-dialog | 2.8.2 | MIT or Apache-2.0 | Open and save dialogs |
| serde, serde_json, thiserror, uuid, sha2, hex, base64 | crates.io | MIT or Apache-2.0 | Serialization, ids, cache key, thumbnails |
| React, React DOM | 18.3 | MIT | UI |
| Vite, TypeScript, Vitest, Tailwind, PostCSS, Autoprefixer | dev | MIT | Build and tests |
| @tauri-apps/api, plugin-dialog, cli | 2.12 / 2.8 | MIT or Apache-2.0 | Frontend IPC |

MIT and Apache-2.0 work can be included in a GPL-3.0-or-later program. `Cargo.lock` and `apps/desktop/pnpm-lock.yaml` are the reproducible sets. Transitive crates are not all named here. `cargo deny` or a generated SBOM is Phase 5 work and has not been run.

## FFmpeg

PulseCut runs `ffmpeg` and `ffprobe` from `PATH`, or `PULSECUT_FFMPEG` / `PULSECUT_FFPROBE`. It does not link libav and does not commit FFmpeg binaries.

The Ubuntu 24.04 package `ffmpeg` 6.1.1-3ubuntu5 reports `--enable-gpl` and is therefore GPL-licensed as a whole, because GPL-only pieces such as libx264 are enabled. An LGPL FFmpeg build is also a valid external tool if it still provides libx264 or another encoder PulseCut is taught to use. Today the renderer requires libx264 and an AAC encoder.

Distributing PulseCut without an FFmpeg binary does not distribute FFmpeg. Distributing a GPL FFmpeg build later requires that build's license, source offer, and notices, and it must stay a separate program or the combined distribution must follow the GPL. Do not ship a non-redistributable codec pack.

H.264 patents are a distribution constraint in some countries even when the software license allows the code. This repository does not ship libx264.

## MLT

melt 7.22.0 was installed to run the spike. It is not a Cargo or npm dependency. MLT framework is LGPL-2.1-or-later. The melt CLI is GPL-2.0-or-later. Module licenses differ, including GPL modules. Do not copy MLT modules into a release without reading that module's license.

## Fonts, media, and telemetry

No font files, icon packs, or LUTs are bundled. The UI asks the OS for Segoe UI, Inter, or Liberation Sans, then `system-ui`. Fixture media under `fixtures/media` is synthetic color and sine tones produced by `scripts/generate_fixtures.sh`. It is safe to redistribute. There is no telemetry.

## Source obligations now

Anyone who distributes a modified PulseCut binary must follow GPL-3.0: ship the corresponding source of PulseCut, keep the license, and preserve notices. They do not have to ship FFmpeg source unless they ship FFmpeg.

## Chosen license

GPL-3.0-or-later is the OSI-approved copyleft license that stays compatible with the MIT/Apache UI stack and with a future GPL FFmpeg bundle. An LGPL application license was rejected because a bundled GPL FFmpeg would not be a clean fit, and the project preference was strong copyleft once that check was made.
