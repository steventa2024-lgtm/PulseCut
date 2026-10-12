import type { Rational } from "./types";

export const toNumber = (r: Rational) => r.num / r.den;

/** Same convention as `format_timecode` in crates/project-model (non-drop-frame). */
export function formatTimecode(frame: number, fps: Rational): string {
  const nominal = Math.max(1, Math.ceil(fps.num / fps.den));
  const sign = frame < 0 ? "-" : "";
  const f = Math.abs(Math.trunc(frame));
  const ff = f % nominal;
  const s = Math.floor(f / nominal);
  const pad = (v: number) => String(v).padStart(2, "0");
  return `${sign}${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}:${pad(ff)}`;
}

export function framesToSeconds(frames: number, fps: Rational): number {
  return (frames * fps.den) / fps.num;
}

export function secondsToFrames(seconds: number, fps: Rational): number {
  return Math.floor((seconds * fps.num) / fps.den + 1e-3);
}

export function formatFps(fps: Rational): string {
  const v = fps.num / fps.den;
  return Number.isInteger(v) ? `${v}` : v.toFixed(3).replace(/0+$/, "");
}

export function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds - m * 60;
  return `${m}:${s.toFixed(2).padStart(5, "0")}`;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

/** Ruler tick spacing (in frames) so labels stay at least `minPx` apart. */
export function rulerStep(pxPerFrame: number, fps: Rational, minPx = 80): { major: number; minor: number } {
  const nominal = Math.max(1, Math.ceil(fps.num / fps.den));
  const candidates = [1, 2, 5, 10, nominal / 2, nominal, nominal * 2, nominal * 5, nominal * 10, nominal * 30, nominal * 60, nominal * 300, nominal * 600]
    .filter((c) => Number.isInteger(c) && c > 0)
    .sort((a, b) => a - b);
  const major = candidates.find((c) => c * pxPerFrame >= minPx) ?? candidates[candidates.length - 1];
  const minorDiv = major >= nominal ? 5 : major >= 5 ? 5 : major;
  return { major, minor: Math.max(1, Math.round(major / minorDiv)) };
}
