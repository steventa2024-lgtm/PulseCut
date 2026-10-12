import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  EditCommand,
  EditOutcome,
  EngineStatus,
  ExportEvent,
  ExportSettings,
  ProjectState,
  RecoveryInfo,
  Uuid,
} from "./types";

export interface ImportResult {
  state: ProjectState;
  imported: Uuid[];
  failures: { path: string; message: string }[];
}

export const ipc = {
  engineStatus: () => invoke<EngineStatus>("engine_status"),
  getState: () => invoke<ProjectState>("get_state"),
  newProject: (args: { name: string; width: number; height: number; fpsNum: number; fpsDen: number }) =>
    invoke<ProjectState>("new_project", { args }),
  importMedia: (paths: string[]) => invoke<ImportResult>("import_media", { paths }),
  applyEdit: (command: EditCommand) => invoke<{ state: ProjectState; outcome: EditOutcome }>("apply_edit", { command }),
  undo: () => invoke<ProjectState>("undo"),
  redo: () => invoke<ProjectState>("redo"),
  removeAsset: (assetId: Uuid) => invoke<ProjectState>("remove_asset", { assetId }),
  relinkAsset: (assetId: Uuid, path: string) => invoke<ProjectState>("relink_asset", { assetId, path }),
  saveProject: (path: string | null) => invoke<ProjectState>("save_project", { path }),
  openProject: (path: string) => invoke<{ state: ProjectState; relinked: Uuid[] }>("open_project", { path }),
  checkRecovery: () => invoke<RecoveryInfo | null>("check_recovery"),
  recoverAutosave: () => invoke<ProjectState>("recover_autosave"),
  discardRecovery: () => invoke<void>("discard_recovery"),
  renderFrame: (frame: number, scale: number) => invoke<ArrayBuffer>("render_frame", { frame, scale }),
  renderAudio: (startFrame: number, endFrame: number) => invoke<ArrayBuffer>("render_audio", { startFrame, endFrame }),
  playbackStart: (frame: number, scale: number, onFrame: Channel<ArrayBuffer>) =>
    invoke<void>("playback_start", { frame, scale, onFrame }),
  playbackStop: () => invoke<void>("playback_stop"),
  assetThumbnail: (assetId: Uuid) => invoke<ArrayBuffer>("asset_thumbnail", { assetId }),
  assetWaveform: (assetId: Uuid) => invoke<{ peaksPerSecond: number; peaks: number[] }>("asset_waveform", { assetId }),
  exportStart: (path: string, settings: ExportSettings, onEvent: Channel<ExportEvent>) =>
    invoke<void>("export_start", { path, settings, onEvent }),
  exportCancel: () => invoke<boolean>("export_cancel"),
};

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
