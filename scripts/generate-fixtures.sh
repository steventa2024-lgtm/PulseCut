#!/usr/bin/env bash
# Regenerates PulseCut's synthetic, license-safe test media using only FFmpeg's
# built-in lavfi sources (no third-party footage). Output is committed under
# fixtures/media so tests are deterministic; rerun only to change fixtures.
set -euo pipefail
OUT="$(cd "$(dirname "$0")/.." && pwd)/fixtures/media"
mkdir -p "$OUT"
FF="${PULSECUT_FFMPEG:-ffmpeg}"
common=(-hide_banner -loglevel error -y -bitexact)

# 4 s, 640x360 @ 30 fps, H.264 + AAC 48 kHz stereo 440 Hz tone. Burned-in frame counter.
"$FF" "${common[@]}" -f lavfi -i "testsrc2=size=640x360:rate=30:duration=4" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=4" \
  -vf "drawbox=x=0:y=0:w=640:h=40:color=black@0.6:t=fill" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -g 30 -crf 23 \
  -c:a aac -b:a 96k -ac 2 -map_metadata -1 "$OUT/clip_a_640x360_30fps.mp4"

# 3 s, 640x360 @ 25 fps (mixed frame rate on purpose), MOV, H.264 + AAC 880 Hz tone.
"$FF" "${common[@]}" -f lavfi -i "smptehdbars=size=640x360:rate=25:duration=3" \
  -f lavfi -i "sine=frequency=880:sample_rate=44100:duration=3" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -g 25 -crf 23 \
  -c:a aac -b:a 96k -ac 2 -map_metadata -1 "$OUT/clip_b_640x360_25fps.mov"

# 6 s stereo music-bed stand-in: two-tone chord, 48 kHz PCM WAV.
"$FF" "${common[@]}" -f lavfi -i "sine=frequency=220:sample_rate=48000:duration=6" \
  -f lavfi -i "sine=frequency=330:sample_rate=48000:duration=6" \
  -filter_complex "[0][1]amix=inputs=2:normalize=0,volume=0.4,aformat=channel_layouts=stereo" \
  -c:a pcm_s16le -map_metadata -1 "$OUT/music_bed_48k.wav"

# Deliberately corrupt file for error-path tests.
printf 'PULSECUT-NOT-A-VIDEO\x00\x01\x02' > "$OUT/corrupt.mp4"
echo "fixtures written to $OUT"
ls -la "$OUT"
