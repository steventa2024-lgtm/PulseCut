import { create } from "zustand";
import { open, save, ask } from "@tauri-apps/plugin-dialog";
import { errorMessage, ipc } from "./ipc";
import type { EditCommand, EngineStatus, ProjectState, Uuid } from "./types";
import { playback } from "./playback";
import { withExtension } from "./paths";

export type Dialog = null | "new" | "export" | "shortcuts" | "about" | "status";

export interface Toast {
  id: number;
  kind: "info" | "error" | "ok";
  text: string;
}

interface Store {
  ps: ProjectState | null;
  engine: EngineStatus | null;
  playhead: number;
  playing: boolean;
  droppedFrames: number;
  selection: Uuid[];
  selectedAsset: Uuid | null;
  pxPerFrame: number;
  snap: boolean;
  previewScale: number;
  safeAreas: boolean;
  reducedMotion: boolean;
  dialog: Dialog;
  toasts: Toast[];
  busy: string | null;

  init: () => Promise<void>;
  setState: (ps: ProjectState) => void;
  toast: (kind: Toast["kind"], text: string) => void;
  dismissToast: (id: number) => void;
  setDialog: (d: Dialog) => void;
  setPlayhead: (f: number) => void;
  setSelection: (ids: Uuid[]) => void;
  selectAsset: (id: Uuid | null) => void;
  setZoom: (ppf: number) => void;
  toggleSnap: () => void;
  setPreviewScale: (s: number) => void;
  toggleSafeAreas: () => void;
  toggleReducedMotion: () => void;

  edit: (cmd: EditCommand) => Promise<boolean>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  importFiles: (paths?: string[]) => Promise<void>;
  saveProject: (saveAs?: boolean) => Promise<boolean>;
  openProject: () => Promise<void>;
  confirmDiscard: () => Promise<boolean>;
  relink: (assetId: Uuid) => Promise<void>;
  removeAsset: (assetId: Uuid) => Promise<void>;
  togglePlay: () => Promise<void>;
  stopPlayback: () => Promise<void>;
  splitAtPlayhead: () => Promise<void>;
  deleteSelection: (ripple: boolean) => Promise<void>;
  addAssetAtPlayhead: (assetId: Uuid) => Promise<void>;
}

const LS = "pulsecut.ui.v1";
const saved: Partial<Pick<Store, "snap" | "previewScale" | "safeAreas" | "reducedMotion" | "pxPerFrame">> = (() => {
  try {
    return JSON.parse(localStorage.getItem(LS) ?? "{}");
  } catch {
    return {};
  }
})();

function persist(s: Store) {
  const { snap, previewScale, safeAreas, reducedMotion, pxPerFrame } = s;
  localStorage.setItem(LS, JSON.stringify({ snap, previewScale, safeAreas, reducedMotion, pxPerFrame }));
}

let toastId = 1;

export const MEDIA_FILTER = {
  name: "Media",
  extensions: ["mp4", "mov", "m4v", "mkv", "webm", "avi", "mxf", "mts", "m2ts", "wav", "mp3", "aac", "m4a", "flac", "ogg", "opus", "png", "jpg", "jpeg", "webp", "bmp"],
};

export const useStore = create<Store>((set, get) => ({
  ps: null,
  engine: null,
  playhead: 0,
  playing: false,
  droppedFrames: 0,
  selection: [],
  selectedAsset: null,
  pxPerFrame: saved.pxPerFrame ?? 3,
  snap: saved.snap ?? true,
  previewScale: saved.previewScale ?? 0.5,
  safeAreas: saved.safeAreas ?? false,
  reducedMotion: saved.reducedMotion ?? false,
  dialog: null,
  toasts: [],
  busy: null,

  init: async () => {
    const [engine, ps] = await Promise.all([ipc.engineStatus(), ipc.getState()]);
    set({ engine, ps });
    if (!engine.ok) get().toast("error", engine.error ?? "FFmpeg is not available");
    const rec = await ipc.checkRecovery();
    if (rec) {
      const when = rec.modified ? new Date(rec.modified * 1000).toLocaleString() : "an earlier session";
      const yes = await ask(`PulseCut found unsaved changes to "${rec.projectName}" from ${when}. Recover them?`, {
        title: "Recover unsaved project",
        kind: "warning",
        okLabel: "Recover",
        cancelLabel: "Discard",
      });
      if (yes) {
        get().setState(await ipc.recoverAutosave());
        get().toast("ok", "Recovered unsaved changes. Save to keep them.");
      } else {
        await ipc.discardRecovery();
      }
    }
  },

  setState: (ps) => {
    const ids = new Set(ps.project.sequence.tracks.flatMap((t) => t.clips.map((c) => c.id)));
    const end = Math.max(0, ps.project.sequence.tracks.reduce((m, t) => Math.max(m, ...t.clips.map((c) => c.start + c.duration)), 0));
    set((s) => ({
      ps,
      selection: s.selection.filter((id) => ids.has(id)),
      selectedAsset: s.selectedAsset && ps.project.assets.some((a) => a.id === s.selectedAsset) ? s.selectedAsset : null,
      playhead: Math.min(s.playhead, end),
    }));
  },

  toast: (kind, text) => {
    const id = toastId++;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { id, kind, text }] }));
    setTimeout(() => get().dismissToast(id), kind === "error" ? 9000 : 4000);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  setDialog: (dialog) => set({ dialog }),
  setPlayhead: (f) => set({ playhead: Math.max(0, Math.round(f)) }),
  setSelection: (selection) => set({ selection }),
  selectAsset: (selectedAsset) => set({ selectedAsset }),
  setZoom: (ppf) => {
    set({ pxPerFrame: Math.min(40, Math.max(0.02, ppf)) });
    persist(get());
  },
  toggleSnap: () => {
    set((s) => ({ snap: !s.snap }));
    persist(get());
  },
  setPreviewScale: (previewScale) => {
    set({ previewScale });
    persist(get());
  },
  toggleSafeAreas: () => {
    set((s) => ({ safeAreas: !s.safeAreas }));
    persist(get());
  },
  toggleReducedMotion: () => {
    set((s) => ({ reducedMotion: !s.reducedMotion }));
    persist(get());
  },

  edit: async (cmd) => {
    if (get().playing) await get().stopPlayback();
    try {
      const { state, outcome } = await ipc.applyEdit(cmd);
      get().setState(state);
      if (outcome.created.length) set({ selection: outcome.created });
      return true;
    } catch (e) {
      get().toast("error", errorMessage(e));
      return false;
    }
  },
  undo: async () => {
    if (get().playing) await get().stopPlayback();
    get().setState(await ipc.undo());
  },
  redo: async () => {
    if (get().playing) await get().stopPlayback();
    get().setState(await ipc.redo());
  },

  importFiles: async (paths) => {
    let files = paths;
    if (!files) {
      const picked = await open({ multiple: true, title: "Import media", filters: [MEDIA_FILTER] });
      if (!picked) return;
      files = Array.isArray(picked) ? picked : [picked];
    }
    if (!files.length) return;
    set({ busy: `Importing ${files.length} file${files.length > 1 ? "s" : ""}…` });
    try {
      const r = await ipc.importMedia(files);
      get().setState(r.state);
      if (r.imported.length) {
        set({ selectedAsset: r.imported[0] });
        get().toast("ok", `Imported ${r.imported.length} file${r.imported.length > 1 ? "s" : ""}`);
      }
      for (const f of r.failures) get().toast("error", `${f.path.split(/[\\/]/).pop()}: ${f.message}`);
    } catch (e) {
      get().toast("error", errorMessage(e));
    } finally {
      set({ busy: null });
    }
  },

  saveProject: async (saveAs = false) => {
    const ps = get().ps;
    if (!ps) return false;
    let path: string | null = saveAs ? null : ps.path;
    if (!path) {
      const picked = await save({
        title: "Save project",
        defaultPath: `${ps.project.name}.pulsecut`,
        filters: [{ name: "PulseCut project", extensions: ["pulsecut"] }],
      });
      if (!picked) return false;
      path = withExtension(picked, ["pulsecut"]);
    }
    try {
      get().setState(await ipc.saveProject(path));
      get().toast("ok", `Saved ${path.split(/[\\/]/).pop()}`);
      return true;
    } catch (e) {
      get().toast("error", `Save failed: ${errorMessage(e)}`);
      return false;
    }
  },

  confirmDiscard: async () => {
    if (!get().ps?.history.dirty) return true;
    const yes = await ask("The current project has unsaved changes. Discard them?", {
      title: "Unsaved changes",
      kind: "warning",
      okLabel: "Discard changes",
      cancelLabel: "Cancel",
    });
    if (yes) await ipc.discardRecovery();
    return yes;
  },

  openProject: async () => {
    if (!(await get().confirmDiscard())) return;
    const picked = await open({ multiple: false, title: "Open project", filters: [{ name: "PulseCut project", extensions: ["pulsecut"] }] });
    if (!picked || Array.isArray(picked)) return;
    try {
      await get().stopPlayback();
      const r = await ipc.openProject(picked);
      set({ selection: [], playhead: 0, selectedAsset: null });
      get().setState(r.state);
      if (r.relinked.length) get().toast("info", `Relinked ${r.relinked.length} moved media file(s) via project-relative paths`);
      if (r.state.offline.length) get().toast("error", `${r.state.offline.length} media file(s) are offline. Use Relink in the Media panel.`);
    } catch (e) {
      get().toast("error", `Could not open project: ${errorMessage(e)}`);
    }
  },

  relink: async (assetId) => {
    const picked = await open({ multiple: false, title: "Locate media file", filters: [MEDIA_FILTER] });
    if (!picked || Array.isArray(picked)) return;
    try {
      get().setState(await ipc.relinkAsset(assetId, picked));
      get().toast("ok", "Media relinked");
    } catch (e) {
      get().toast("error", `Relink failed: ${errorMessage(e)}`);
    }
  },

  removeAsset: async (assetId) => {
    try {
      get().setState(await ipc.removeAsset(assetId));
    } catch (e) {
      get().toast("error", errorMessage(e));
    }
  },

  togglePlay: async () => {
    if (get().playing) return get().stopPlayback();
    const ps = get().ps;
    if (!ps || !get().engine?.ok) return;
    const end = ps.project.sequence.tracks.reduce((m, t) => Math.max(m, ...t.clips.map((c) => c.start + c.duration)), 0);
    if (end === 0) return;
    let from = get().playhead;
    if (from >= end - 1) from = 0;
    set({ playing: true, droppedFrames: 0, playhead: from });
    try {
      await playback.start(ps.project.sequence, from, end, get().previewScale, {
        onTime: (f) => set({ playhead: f }),
        onDropped: (n) => set({ droppedFrames: n }),
        onEnd: (err) => {
          set({ playing: false });
          if (err) get().toast("error", `Playback error: ${err}`);
        },
      });
    } catch (e) {
      set({ playing: false });
      get().toast("error", `Playback failed: ${errorMessage(e)}`);
    }
  },
  stopPlayback: async () => {
    await playback.stop();
    set({ playing: false });
  },

  splitAtPlayhead: async () => {
    const { playhead, selection } = get();
    await get().edit({ type: "split", frame: playhead, clipIds: selection });
  },
  deleteSelection: async (ripple) => {
    const sel = get().selection;
    if (!sel.length) return;
    await get().edit({ type: "deleteClips", clipIds: sel, ripple });
  },
  addAssetAtPlayhead: async (assetId) => {
    const ps = get().ps;
    if (!ps) return;
    const asset = ps.project.assets.find((a) => a.id === assetId);
    if (!asset) return;
    const kind = asset.media.video ? "video" : "audio";
    const track = ps.project.sequence.tracks.find((t) => t.kind === kind && !t.locked);
    if (!track) return get().toast("error", `No unlocked ${kind} track`);
    await get().edit({ type: "addClip", assetId, trackId: track.id, start: get().playhead, includeLinkedAudio: true });
  },
}));

export function sequenceEnd(ps: ProjectState | null): number {
  if (!ps) return 0;
  return ps.project.sequence.tracks.reduce((m, t) => Math.max(m, ...t.clips.map((c) => c.start + c.duration)), 0);
}
