import { describe, expect, it } from "vitest";
import { formatTimecode, rulerStep, secondsToFrames } from "./time";
import { snapPoint, snapRange, snapTargets } from "./snap";
import type { Sequence } from "./types";

describe("timecode", () => {
  it("matches the Rust formatter", () => {
    expect(formatTimecode(0, { num: 30, den: 1 })).toBe("00:00:00:00");
    expect(formatTimecode(30 * 3661 + 7, { num: 30, den: 1 })).toBe("01:01:01:07");
    expect(formatTimecode(31, { num: 30000, den: 1001 })).toBe("00:00:01:01");
  });
  it("converts seconds to whole frames with tolerance", () => {
    expect(secondsToFrames(2, { num: 30, den: 1 })).toBe(60);
    expect(secondsToFrames(1 / 3, { num: 30, den: 1 })).toBe(10);
  });
  it("chooses readable ruler steps", () => {
    expect(rulerStep(4, { num: 30, den: 1 }).major).toBe(30);
    expect(rulerStep(0.1, { num: 30, den: 1 }).major).toBeGreaterThanOrEqual(300);
  });
});

const seq: Sequence = {
  id: "s",
  name: "S",
  frameRate: { num: 30, den: 1 },
  width: 1920,
  height: 1080,
  sampleRate: 48000,
  tracks: [
    {
      id: "t",
      kind: "video",
      name: "V1",
      muted: false,
      locked: false,
      solo: false,
      clips: [
        { id: "a", assetId: "x", start: 0, duration: 100, sourceIn: { num: 0, den: 1 }, link: null, params: { opacity: 1, scale: 1, positionX: 0, positionY: 0, gainDb: 0 } },
        { id: "b", assetId: "x", start: 200, duration: 50, sourceIn: { num: 0, den: 1 }, link: null, params: { opacity: 1, scale: 1, positionX: 0, positionY: 0, gainDb: 0 } },
      ],
    },
  ],
};

describe("snapping", () => {
  it("collects edges excluding dragged clips", () => {
    expect(snapTargets(seq, 42, new Set(["b"]))).toEqual([0, 42, 100]);
  });
  it("snaps either edge of a moving range", () => {
    const targets = snapTargets(seq, 0, new Set(["b"]));
    expect(snapRange(103, 50, targets, 5)).toEqual({ start: 100, snappedTo: 100 });
    expect(snapRange(52, 50, targets, 5)).toEqual({ start: 50, snappedTo: 100 });
    expect(snapRange(130, 50, targets, 5)).toEqual({ start: 130, snappedTo: null });
  });
  it("snaps points", () => {
    expect(snapPoint(198, [0, 200], 3)).toEqual({ frame: 200, snappedTo: 200 });
  });
});
