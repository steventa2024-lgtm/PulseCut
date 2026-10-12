export type TrackKind = "video" | "audio";

export type Asset = {
  id: string;
  name: string;
  path: string;
  relativePath: string | null;
  durationUs: number;
  width: number | null;
  height: number | null;
  fpsNum: number | null;
  fpsDen: number | null;
  hasVideo: boolean;
  hasAudio: boolean;
  audioSampleRate: number | null;
  audioChannels: number | null;
  videoCodec: string | null;
  audioCodec: string | null;
  formatName: string;
  vfrSuspected: boolean;
  hdr: boolean;
  offline: boolean;
};

export type Clip = {
  id: string;
  assetId: string;
  timelineStartUs: number;
  sourceInUs: number;
  durationUs: number;
  name: string;
  linkId: string | null;
  gainDb: number;
};

export type Track = {
  id: string;
  name: string;
  kind: TrackKind;
  muted: boolean;
  locked: boolean;
  clips: Clip[];
};

export type Sequence = {
  id: string;
  name: string;
  width: number;
  height: number;
  fpsNum: number;
  fpsDen: number;
  tracks: Track[];
};

export type Project = {
  schemaVersion: number;
  id: string;
  name: string;
  assets: Asset[];
  sequence: Sequence;
};

export type UiState = {
  project: Project;
  revision: number;
  canUndo: boolean;
  canRedo: boolean;
  dirty: boolean;
  path: string | null;
};

export type EngineReport = {
  ok: boolean;
  message: string;
  ffmpegPath: string | null;
  ffprobePath: string | null;
  version: string | null;
  libx264: boolean;
  aac: boolean;
};

export type AppInfo = {
  name: string;
  version: string;
  license: string;
  previewBase: string;
};

export type RecoveryInfo = {
  exists: boolean;
  savedAtUnixMs: number | null;
  projectName: string | null;
  projectPath: string | null;
};

export type RenderDone = {
  mode: string;
  revision: number;
  output: string;
  durationUs: number;
  width: number;
  height: number;
  bytes: number;
  videoCodec: string | null;
  audioCodec: string | null;
  fpsNum: number | null;
  fpsDen: number | null;
  audioSampleRate: number | null;
  audioChannels: number | null;
};

export type RenderProgress = {
  mode: string;
  ratio: number;
  message: string;
};

export type Command =
  | { type: "importAsset"; asset: Asset }
  | { type: "updateAsset"; asset: Asset }
  | { type: "removeAsset"; assetId: string }
  | {
      type: "placeAsset";
      assetId: string;
      videoTrackId: string | null;
      audioTrackId: string | null;
      timelineStartUs: number;
      sourceInUs: number;
      durationUs: number | null;
    }
  | { type: "moveClip"; clipId: string; timelineStartUs: number }
  | { type: "trimClip"; clipId: string; edge: "start" | "end"; timelineUs: number }
  | {
      type: "setClipTiming";
      clipId: string;
      timelineStartUs: number;
      sourceInUs: number;
      durationUs: number;
    }
  | { type: "setClipName"; clipId: string; name: string }
  | { type: "setClipGain"; clipId: string; gainDb: number }
  | { type: "splitClip"; clipId: string; timelineUs: number }
  | { type: "deleteClip"; clipId: string }
  | { type: "unlinkClip"; clipId: string }
  | { type: "setTrackMuted"; trackId: string; muted: boolean }
  | { type: "setTrackLocked"; trackId: string; locked: boolean }
  | { type: "addTrack"; kind: TrackKind; name?: string | null }
  | {
      type: "setSequence";
      name?: string | null;
      width: number;
      height: number;
      fpsNum: number;
      fpsDen: number;
    }
  | { type: "undo" }
  | { type: "redo" };

export type RenderRequest = {
  mode: "preview" | "export";
  output?: string | null;
  width?: number | null;
  height?: number | null;
  crf?: number | null;
  preset?: string | null;
  audioBitrateKbps?: number | null;
};
