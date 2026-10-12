export function frameIndex(timeUs: number, fpsNum: number, fpsDen: number): number {
  if (timeUs <= 0 || fpsNum <= 0 || fpsDen <= 0) return 0;
  return Math.floor((timeUs * fpsNum) / (1_000_000 * fpsDen));
}

export function frameStartUs(frame: number, fpsNum: number, fpsDen: number): number {
  if (frame <= 0 || fpsNum <= 0 || fpsDen <= 0) return 0;
  return Math.floor((frame * 1_000_000 * fpsDen) / fpsNum);
}

export function formatTimecode(timeUs: number, fpsNum: number, fpsDen: number): string {
  const nominal = Math.max(1, Math.round(fpsNum / fpsDen));
  const frame = frameIndex(Math.max(0, timeUs), fpsNum, fpsDen);
  const ff = frame % nominal;
  const totalSeconds = Math.floor(frame / nominal);
  const ss = totalSeconds % 60;
  const mm = Math.floor(totalSeconds / 60) % 60;
  const hh = Math.floor(totalSeconds / 3600);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(hh)}:${pad(mm)}:${pad(ss)}:${pad(ff)}`;
}

export function parseTimecode(text: string, fpsNum: number, fpsDen: number): number | null {
  const trimmed = text.trim();
  const match = trimmed.match(/^(\d+):(\d+):(\d+):(\d+)$/);
  if (!match) {
    const seconds = Number(trimmed);
    if (!Number.isFinite(seconds) || seconds < 0) return null;
    return Math.round(seconds * 1_000_000);
  }
  const hh = Number(match[1]);
  const mm = Number(match[2]);
  const ss = Number(match[3]);
  const ff = Number(match[4]);
  const nominal = Math.max(1, Math.round(fpsNum / fpsDen));
  if (mm >= 60 || ss >= 60 || ff >= nominal) return null;
  const frame = ((hh * 60 + mm) * 60 + ss) * nominal + ff;
  return frameStartUs(frame, fpsNum, fpsDen);
}

export function sequenceDurationUs(tracks: { clips: { timelineStartUs: number; durationUs: number }[] }[]): number {
  let end = 0;
  for (const track of tracks) {
    for (const clip of track.clips) {
      end = Math.max(end, clip.timelineStartUs + clip.durationUs);
    }
  }
  return end;
}

export function snapUs(
  timeUs: number,
  points: number[],
  thresholdUs: number,
  fpsNum: number,
  fpsDen: number,
): { timeUs: number; snappedTo: number | null } {
  const framed = frameStartUs(frameIndex(Math.max(0, timeUs), fpsNum, fpsDen), fpsNum, fpsDen);
  let best: number | null = null;
  let bestDist = thresholdUs;
  for (const point of points) {
    const dist = Math.abs(point - timeUs);
    if (dist <= bestDist) {
      best = point;
      bestDist = dist;
    }
  }
  if (best !== null) {
    return {
      timeUs: frameStartUs(frameIndex(Math.max(0, best), fpsNum, fpsDen), fpsNum, fpsDen),
      snappedTo: best,
    };
  }
  return { timeUs: framed, snappedTo: null };
}
