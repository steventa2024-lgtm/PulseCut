/**
 * Ensures `path` ends with exactly one of `exts` (the first is the default).
 * GTK save dialogs preselect only the stem, so typing "name.ext" over it yields
 * "name.ext.ext"; repeated trailing extensions are collapsed.
 */
export function withExtension(path: string, exts: string[]): string {
  const re = new RegExp(`(\\.(${exts.join("|")}))+$`, "i");
  const m = path.match(re);
  if (!m) return `${path}.${exts[0]}`;
  return path.slice(0, m.index) + `.${m[2]}`;
}
