#!/usr/bin/env bash
# Synthetic media for PulseCut samples and manual checks.
# Generated locally with FFmpeg. No third-party footage or audio.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/fixtures/media"
mkdir -p "$OUT"

ffmpeg -y -hide_banner -loglevel error \
  -f lavfi -i "color=c=0xE23B3B:s=1280x720:r=30:d=5,format=yuv420p" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=5" \
  -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest \
  "$OUT/clip_red_720p30.mp4"

ffmpeg -y -hide_banner -loglevel error \
  -f lavfi -i "color=c=0x2F6FED:s=1920x1080:r=24:d=4,format=yuv420p" \
  -f lavfi -i "sine=frequency=880:sample_rate=44100:duration=4" \
  -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest \
  "$OUT/clip_blue_1080p24.mp4"

ffmpeg -y -hide_banner -loglevel error \
  -f lavfi -i "sine=frequency=220:sample_rate=48000:duration=8" \
  -f lavfi -i "sine=frequency=330:sample_rate=48000:duration=8" \
  -filter_complex "[0:a][1:a]amix=inputs=2:normalize=0,aformat=channel_layouts=stereo" \
  -c:a aac -b:a 160k \
  "$OUT/music_stereo.m4a"

echo "Wrote fixtures to $OUT"
