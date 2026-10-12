# Fixtures

These files are generated. They are solid colors and sine tones. No recorded footage or music is included.

Regenerate from the repository root:

```bash
bash scripts/generate_fixtures.sh
```

Requires `ffmpeg` with libx264 and AAC.

| File | Picture | Audio | Duration |
| --- | --- | --- | --- |
| `clip_red_720p30.mp4` | 1280×720, 30 fps, H.264, color `0xE23B3B` | AAC, 48 kHz, mono, 440 Hz | 5.000 s |
| `clip_blue_1080p24.mp4` | 1920×1080, 24 fps, H.264, color `0x2F6FED` | AAC, 44.1 kHz, mono, 880 Hz | 4.000 s |
| `music_stereo.m4a` | none | AAC, 48 kHz, stereo, 220 Hz + 330 Hz | 8.000 s |

Sizes on the machine that generated the committed copies: red 57348 bytes, blue 47003 bytes, music 162631 bytes. A rebuild can change byte size. Probed durations and frame sizes should not.

File → Load sample timeline places the red clip on V1 and A1 from the start, the blue clip on V2 from 2.000 s with a 0.500 s source in and a 2.000 s duration, and the music on A2 from the start for 6.000 s. Expected picture on a 1280×720 30 fps sequence: red at 1 s, blue on top at 3 s, red at 4.5 s, black at 5.5 s while the music continues.

The automated pipeline test synthesizes its own files under the temp directory. It does not read this folder.
