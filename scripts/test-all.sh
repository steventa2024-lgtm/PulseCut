#!/usr/bin/env bash
# Runs every automated check: Rust tests + lints and the desktop frontend checks.
set -euo pipefail
cd "$(dirname "$0")/.."

command -v ffmpeg >/dev/null && command -v ffprobe >/dev/null || {
  echo "error: ffmpeg/ffprobe not found on PATH (end-to-end tests need them)" >&2
  exit 1
}

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd apps/desktop
[ -d node_modules ] || npm ci
npm run typecheck
npm test
npm run build
