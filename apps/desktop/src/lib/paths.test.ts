import { describe, expect, it } from "vitest";
import { withExtension } from "./paths";

describe("withExtension", () => {
  it("adds the default extension when missing", () => {
    expect(withExtension("/tmp/a", ["pulsecut"])).toBe("/tmp/a.pulsecut");
  });
  it("keeps a single extension", () => {
    expect(withExtension("/tmp/a.MOV", ["mp4", "mov"])).toBe("/tmp/a.MOV");
  });
  it("collapses repeated extensions from GTK save dialogs", () => {
    expect(withExtension("/tmp/a.pulsecut.pulsecut", ["pulsecut"])).toBe("/tmp/a.pulsecut");
    expect(withExtension("C:\\v\\out.mp4.mp4", ["mp4", "mov"])).toBe("C:\\v\\out.mp4");
  });
  it("does not treat a stem containing the extension as repeated", () => {
    expect(withExtension("/tmp/my.pulsecut-notes", ["pulsecut"])).toBe("/tmp/my.pulsecut-notes.pulsecut");
  });
});
