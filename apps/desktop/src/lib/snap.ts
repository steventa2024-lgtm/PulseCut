import type { Sequence, Uuid } from "./types";

/** Frame positions a dragged edge can snap to: 0, playhead and every clip edge. */
export function snapTargets(seq: Sequence, playhead: number, exclude: Set<Uuid>): number[] {
  const set = new Set<number>([0, playhead]);
  for (const t of seq.tracks) {
    for (const c of t.clips) {
      if (exclude.has(c.id)) continue;
      set.add(c.start);
      set.add(c.start + c.duration);
    }
  }
  return [...set].sort((a, b) => a - b);
}

/**
 * Snaps a moving range [start, start+length) so that either edge lands on the nearest
 * target within `thresholdFrames`. Returns the adjusted start and the target hit.
 */
export function snapRange(
  start: number,
  length: number,
  targets: number[],
  thresholdFrames: number,
): { start: number; snappedTo: number | null } {
  let best: { delta: number; target: number } | null = null;
  for (const t of targets) {
    for (const edge of [start, start + length]) {
      const d = t - edge;
      if (Math.abs(d) <= thresholdFrames && (!best || Math.abs(d) < Math.abs(best.delta))) best = { delta: d, target: t };
    }
  }
  return best ? { start: start + best.delta, snappedTo: best.target } : { start, snappedTo: null };
}

export function snapPoint(frame: number, targets: number[], thresholdFrames: number): { frame: number; snappedTo: number | null } {
  let best: number | null = null;
  for (const t of targets) {
    if (Math.abs(t - frame) <= thresholdFrames && (best === null || Math.abs(t - frame) < Math.abs(best - frame))) best = t;
  }
  return best === null ? { frame, snappedTo: null } : { frame: best, snappedTo: best };
}
