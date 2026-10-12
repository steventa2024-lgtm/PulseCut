import { getCurrentWindow } from "@tauri-apps/api/window";
import { sequenceEnd, useStore } from "./store";

export interface Action {
  id: string;
  label: string;
  /** Display + matching form, e.g. "Ctrl+Shift+Z". Multiple alternatives allowed. */
  keys?: string[];
  enabled?: () => boolean;
  run: () => void | Promise<void>;
}

const s = () => useStore.getState();
const hasProject = () => !!s().ps;
const fps = () => {
  const r = s().ps?.project.sequence.frameRate;
  return r ? r.num / r.den : 30;
};
const seek = (f: number) => {
  if (s().playing) void s().stopPlayback();
  s().setPlayhead(Math.max(0, Math.min(sequenceEnd(s().ps), f)));
};

export async function requestQuit() {
  if (await s().confirmDiscard()) await getCurrentWindow().destroy();
}

export const ACTIONS: Record<string, Action> = Object.fromEntries(
  (
    [
      { id: "file.new", label: "New Project…", keys: ["Ctrl+N"], run: async () => (await s().confirmDiscard()) && s().setDialog("new") },
      { id: "file.open", label: "Open Project…", keys: ["Ctrl+O"], run: () => s().openProject() },
      { id: "file.save", label: "Save", keys: ["Ctrl+S"], enabled: hasProject, run: () => void s().saveProject(false) },
      { id: "file.saveAs", label: "Save As…", keys: ["Ctrl+Shift+S"], enabled: hasProject, run: () => void s().saveProject(true) },
      { id: "file.import", label: "Import Media…", keys: ["Ctrl+I"], enabled: () => !!s().engine?.ok, run: () => s().importFiles() },
      { id: "file.export", label: "Export Video…", keys: ["Ctrl+E"], enabled: () => sequenceEnd(s().ps) > 0 && !!s().engine?.ok, run: () => s().setDialog("export") },
      { id: "file.quit", label: "Quit", keys: ["Ctrl+Q"], run: requestQuit },

      { id: "edit.undo", label: "Undo", keys: ["Ctrl+Z"], enabled: () => !!s().ps?.history.canUndo, run: () => s().undo() },
      { id: "edit.redo", label: "Redo", keys: ["Ctrl+Shift+Z", "Ctrl+Y"], enabled: () => !!s().ps?.history.canRedo, run: () => s().redo() },
      { id: "edit.delete", label: "Delete", keys: ["Delete", "Backspace"], enabled: () => s().selection.length > 0, run: () => s().deleteSelection(false) },
      { id: "edit.rippleDelete", label: "Ripple Delete", keys: ["Shift+Delete", "Shift+Backspace"], enabled: () => s().selection.length > 0, run: () => s().deleteSelection(true) },
      {
        id: "edit.selectAll",
        label: "Select All Clips",
        keys: ["Ctrl+A"],
        enabled: hasProject,
        run: () => s().setSelection(s().ps!.project.sequence.tracks.filter((t) => !t.locked).flatMap((t) => t.clips.map((c) => c.id))),
      },
      { id: "edit.deselect", label: "Deselect All", keys: ["Escape"], run: () => s().setSelection([]) },

      { id: "view.zoomIn", label: "Zoom In Timeline", keys: ["="], run: () => s().setZoom(s().pxPerFrame * 1.4) },
      { id: "view.zoomOut", label: "Zoom Out Timeline", keys: ["-"], run: () => s().setZoom(s().pxPerFrame / 1.4) },
      { id: "view.zoomFit", label: "Zoom Timeline to Fit", keys: ["Shift+Z"], run: () => window.dispatchEvent(new CustomEvent("pulsecut:zoom-fit")) },
      { id: "view.safeAreas", label: "Toggle Safe Areas", keys: ["'"], run: () => s().toggleSafeAreas() },
      { id: "view.qualityFull", label: "Preview Quality: Full", run: () => s().setPreviewScale(1) },
      { id: "view.qualityHalf", label: "Preview Quality: Half", run: () => s().setPreviewScale(0.5) },
      { id: "view.qualityQuarter", label: "Preview Quality: Quarter", run: () => s().setPreviewScale(0.25) },
      { id: "view.reducedMotion", label: "Toggle Reduced Motion", run: () => s().toggleReducedMotion() },

      { id: "play.toggle", label: "Play / Pause", keys: ["Space"], enabled: () => sequenceEnd(s().ps) > 0 && !!s().engine?.ok, run: () => s().togglePlay() },
      { id: "play.start", label: "Go to Start", keys: ["Home"], run: () => seek(0) },
      { id: "play.end", label: "Go to End", keys: ["End"], run: () => seek(Math.max(0, sequenceEnd(s().ps) - 1)) },
      { id: "play.prev", label: "Previous Frame", keys: ["ArrowLeft"], run: () => seek(s().playhead - 1) },
      { id: "play.next", label: "Next Frame", keys: ["ArrowRight"], run: () => seek(s().playhead + 1) },
      { id: "play.back1s", label: "Back 1 Second", keys: ["Shift+ArrowLeft"], run: () => seek(s().playhead - Math.round(fps())) },
      { id: "play.fwd1s", label: "Forward 1 Second", keys: ["Shift+ArrowRight"], run: () => seek(s().playhead + Math.round(fps())) },
      {
        id: "play.prevEdit",
        label: "Previous Edit Point",
        keys: ["ArrowUp"],
        run: () => {
          const pts = editPoints().filter((p) => p < s().playhead);
          if (pts.length) seek(pts[pts.length - 1]);
        },
      },
      {
        id: "play.nextEdit",
        label: "Next Edit Point",
        keys: ["ArrowDown"],
        run: () => {
          const p = editPoints().find((p) => p > s().playhead);
          if (p !== undefined) seek(p);
        },
      },

      { id: "tl.split", label: "Split at Playhead", keys: ["S", "Ctrl+K"], enabled: () => sequenceEnd(s().ps) > 0, run: () => s().splitAtPlayhead() },
      { id: "tl.snap", label: "Toggle Snapping", keys: ["N"], run: () => s().toggleSnap() },
      { id: "tl.addVideo", label: "Add Video Track", enabled: hasProject, run: () => void s().edit({ type: "addTrack", kind: "video" }) },
      { id: "tl.addAudio", label: "Add Audio Track", enabled: hasProject, run: () => void s().edit({ type: "addTrack", kind: "audio" }) },
      {
        id: "tl.insertAsset",
        label: "Add Selected Media at Playhead",
        keys: ["E"],
        enabled: () => !!s().selectedAsset,
        run: () => s().addAssetAtPlayhead(s().selectedAsset!),
      },

      { id: "window.resetLayout", label: "Reset Panel Layout", run: () => window.dispatchEvent(new CustomEvent("pulsecut:reset-layout")) },

      { id: "help.shortcuts", label: "Keyboard Shortcuts", keys: ["?", "Shift+?"], run: () => s().setDialog("shortcuts") },
      { id: "help.status", label: "Feature Status", run: () => s().setDialog("status") },
      { id: "help.about", label: "About PulseCut", run: () => s().setDialog("about") },
    ] satisfies Action[]
  ).map((a) => [a.id, a]),
);

function editPoints(): number[] {
  const ps = s().ps;
  if (!ps) return [];
  const set = new Set<number>([0]);
  for (const t of ps.project.sequence.tracks) for (const c of t.clips) set.add(c.start), set.add(c.start + c.duration);
  return [...set].sort((a, b) => a - b);
}

export function eventKey(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey && e.key.length > 1) parts.push("Shift");
  else if (e.shiftKey && e.key === "?") parts.push("Shift");
  else if (e.shiftKey && /^[a-z]$/i.test(e.key)) parts.push("Shift");
  let key = e.key === " " ? "Space" : e.key;
  if (key.length === 1) key = key.toUpperCase();
  parts.push(key);
  return parts.join("+");
}

export function findAction(e: KeyboardEvent): Action | undefined {
  const k = eventKey(e);
  return Object.values(ACTIONS).find((a) => a.keys?.some((x) => x.toUpperCase() === k.toUpperCase()));
}

export const MENUS: { label: string; items: string[] }[] = [
  { label: "File", items: ["file.new", "file.open", "-", "file.save", "file.saveAs", "-", "file.import", "file.export", "-", "file.quit"] },
  { label: "Edit", items: ["edit.undo", "edit.redo", "-", "edit.delete", "edit.rippleDelete", "-", "edit.selectAll", "edit.deselect"] },
  {
    label: "View",
    items: ["view.zoomIn", "view.zoomOut", "view.zoomFit", "-", "view.safeAreas", "view.qualityFull", "view.qualityHalf", "view.qualityQuarter", "-", "view.reducedMotion"],
  },
  { label: "Playback", items: ["play.toggle", "-", "play.start", "play.end", "play.prev", "play.next", "play.back1s", "play.fwd1s", "play.prevEdit", "play.nextEdit"] },
  { label: "Timeline", items: ["tl.split", "tl.insertAsset", "tl.snap", "-", "tl.addVideo", "tl.addAudio"] },
  { label: "Window", items: ["window.resetLayout"] },
  { label: "Help", items: ["help.shortcuts", "help.status", "-", "help.about"] },
];
