import { describe, expect, it } from "vitest";
import { formatTimecode, frameIndex, frameStartUs, parseTimecode, snapUs } from "./time";

describe("timecode", () => {
  it("lands on exact seconds at 30 fps", () => {
    expect(frameIndex(1_000_000, 30, 1)).toBe(30);
    expect(frameStartUs(30, 30, 1)).toBe(1_000_000);
    expect(formatTimecode(1_000_000, 30, 1)).toBe("00:00:01:00");
    expect(formatTimecode(0, 30, 1)).toBe("00:00:00:00");
  });

  it("round-trips a timecode", () => {
    const us = parseTimecode("00:01:02:15", 30, 1);
    expect(us).toBe(frameStartUs(1 * 60 * 30 + 2 * 30 + 15, 30, 1));
    expect(formatTimecode(us ?? 0, 30, 1)).toBe("00:01:02:15");
  });

  it("rejects a frame number past the rate", () => {
    expect(parseTimecode("00:00:00:30", 30, 1)).toBeNull();
  });

  it("snaps to a nearby clip edge and otherwise to the frame grid", () => {
    const near = snapUs(1_020_000, [1_000_000], 50_000, 30, 1);
    expect(near.snappedTo).toBe(1_000_000);
    expect(near.timeUs).toBe(1_000_000);
    const grid = snapUs(999_999, [], 0, 30, 1);
    expect(grid.snappedTo).toBeNull();
    expect(grid.timeUs).toBe(frameStartUs(29, 30, 1));
  });
});
