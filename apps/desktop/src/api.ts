import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppInfo,
  Asset,
  Command,
  EngineReport,
  RecoveryInfo,
  RenderDone,
  RenderProgress,
  RenderRequest,
  UiState,
} from "./types";

export function desktopShell(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const api = {
  engineReport: () => invoke<EngineReport>("engine_report"),
  appInfo: () => invoke<AppInfo>("app_info"),
  editorState: () => invoke<UiState>("editor_state"),
  dispatch: (command: Command) => invoke<UiState>("dispatch_command", { command }),
  probeMedia: (path: string) => invoke<Asset>("probe_media", { path }),
  newProject: () => invoke<UiState>("new_project"),
  openProject: (path: string) => invoke<UiState>("open_project", { path }),
  saveProject: (path: string) => invoke<UiState>("save_project", { path }),
  startRender: (request: RenderRequest) => invoke<void>("start_render", { request }),
  cancelRender: () => invoke<void>("cancel_render"),
  thumbnail: (path: string, timeUs: number) => invoke<string>("thumbnail", { path, timeUs }),
  waveform: (path: string, sourceInUs: number, durationUs: number, buckets: number) =>
    invoke<number[]>("waveform", { path, sourceInUs, durationUs, buckets }),
  recoveryInfo: () => invoke<RecoveryInfo>("recovery_info"),
  restoreRecovery: () => invoke<UiState>("restore_recovery"),
  discardRecovery: () => invoke<void>("discard_recovery"),
  loadSample: () => invoke<UiState>("load_sample"),
  assetUrl: (path: string) => convertFileSrc(path),
  previewUrl: (path: string, previewBase: string) => {
    const name = path.split(/[/\\]/).pop() ?? "";
    if (previewBase && /^[0-9a-f]{64}\.mp4$/i.test(name)) {
      return `${previewBase.replace(/\/$/, "")}/${name}`;
    }
    return convertFileSrc(path);
  },
  onRenderProgress: (handler: (event: RenderProgress) => void): Promise<UnlistenFn> =>
    listen<RenderProgress>("render-progress", (event) => handler(event.payload)),
  onRenderDone: (handler: (event: RenderDone) => void): Promise<UnlistenFn> =>
    listen<RenderDone>("render-done", (event) => handler(event.payload)),
  onRenderError: (handler: (message: string) => void): Promise<UnlistenFn> =>
    listen<string>("render-error", (event) => handler(event.payload)),
  onRenderCancelled: (handler: () => void): Promise<UnlistenFn> =>
    listen("render-cancelled", () => handler()),
  confirmDiscard: async (dirty: boolean) => {
    if (!dirty) return true;
    return ask("This project has unsaved changes. Continue and discard them?", {
      title: "PulseCut",
      kind: "warning",
    });
  },
  pickMedia: async () => {
    const picked = await open({
      multiple: true,
      title: "Import media",
      filters: [
        {
          name: "Media",
          extensions: ["mp4", "mov", "m4v", "mkv", "webm", "wav", "mp3", "m4a", "aac", "aiff"],
        },
      ],
    });
    if (!picked) return [];
    return Array.isArray(picked) ? picked : [picked];
  },
  pickProject: () =>
    open({
      multiple: false,
      title: "Open PulseCut project",
      filters: [{ name: "PulseCut project", extensions: ["pulsecut"] }],
    }),
  pickProjectSave: (suggested: string) =>
    save({
      title: "Save PulseCut project",
      defaultPath: suggested,
      filters: [{ name: "PulseCut project", extensions: ["pulsecut"] }],
    }),
  pickExport: (suggested: string) =>
    save({
      title: "Export MP4",
      defaultPath: suggested,
      filters: [{ name: "MP4", extensions: ["mp4"] }],
    }),
  pickRelink: () =>
    open({
      multiple: false,
      title: "Relink media",
      filters: [
        {
          name: "Media",
          extensions: ["mp4", "mov", "m4v", "mkv", "webm", "wav", "mp3", "m4a", "aac", "aiff"],
        },
      ],
    }),
};
