#!/usr/bin/env bash
# Same spike timeline as timeline.mlt, expressed as one FFmpeg filter graph.
# Usage: ffmpeg_graph.sh <out> [start_s] [dur_s] [extra output args...]
set -euo pipefail
F=/workspace/fixtures/media; OUT=$1; T0=${2:-0}; D=${3:-4}; shift 3 || shift $#
ffmpeg -hide_banner -loglevel error -y \
  -f lavfi -i "color=c=black:s=640x360:r=30:d=4" \
  -ss 1 -t 2 -i $F/clip_a_640x360_30fps.mp4 \
  -ss 0 -t 2 -i $F/clip_b_640x360_25fps.mov \
  -ss 0 -t 1 -i $F/clip_b_640x360_25fps.mov \
  -i $F/music_bed_48k.wav \
  -f lavfi -i "anullsrc=r=48000:cl=stereo:d=4" \
  -filter_complex "
   [1:v]fps=30,setpts=PTS-STARTPTS+0/TB,format=yuva420p[c1];
   [2:v]fps=30,setpts=PTS-STARTPTS+2/TB,format=yuva420p[c2];
   [3:v]fps=30,scale=320:180,setpts=PTS-STARTPTS+1/TB,format=yuva420p[c3];
   [0:v][c1]overlay=eof_action=pass:enable='between(t,0,2-0.001)'[o1];
   [o1][c2]overlay=eof_action=pass:enable='between(t,2,4-0.001)'[o2];
   [o2][c3]overlay=eof_action=pass:enable='between(t,1,2-0.001)',format=yuv420p,trim=start=$T0:duration=$D,setpts=PTS-STARTPTS[v];
   [1:a]aresample=48000,adelay=0:all=1[a1];[2:a]aresample=48000,adelay=2000:all=1[a2];
   [3:a]aresample=48000,adelay=1000:all=1[a3];[4:a]aresample=48000[a4];
   [5:a][a1][a2][a3][a4]amix=inputs=5:normalize=0:duration=first,atrim=start=$T0:duration=$D,asetpts=PTS-STARTPTS[a]" \
  -map "[v]" -map "[a]" "$@" "$OUT"
