// TypeScript mirror of crates/project-model (serde camelCase). The Rust model is
// authoritative; the UI never mutates these objects, it sends EditCommands.

export type Uuid = string;

export interface Rational {
  num: number;
  den: number;
}

export type MediaKind = "video" | "audio" | "image";
export type TrackKind = "video" | "audio";

export interface VideoStreamInfo {
  codec: string;
  width: number;
  height: number;
  frameRate: Rational;
  pixelFormat: string | null;
  variableFrameRate: boolean;
  rotation: number;
  hdr: boolean;
}

export interface AudioStreamInfo {
  codec: string;
  sampleRate: number;
  channels: number;
}

export interface MediaInfo {
  kind: MediaKind;
  container: string;
  duration: Rational;
  sizeBytes: number;
  video: VideoStreamInfo | null;
  audio: AudioStreamInfo | null;
  audioStreamCount: number;
}

export interface Asset {
  id: Uuid;
  name: string;
  path: string;
  relativePath: string | null;
  media: MediaInfo;
}

export interface ClipParams {
  opacity: number;
  scale: number;
  positionX: number;
  positionY: number;
  gainDb: number;
}

export interface Clip {
  id: Uuid;
  assetId: Uuid;
  start: number;
  duration: number;
  sourceIn: Rational;
  link: Uuid | null;
  params: ClipParams;
}

export interface Track {
  id: Uuid;
  kind: TrackKind;
  name: string;
  muted: boolean;
  locked: boolean;
  solo: boolean;
  clips: Clip[];
}

export interface Sequence {
  id: Uuid;
  name: string;
  frameRate: Rational;
  width: number;
  height: number;
  sampleRate: number;
  tracks: Track[];
}

export interface Project {
  format: string;
  schemaVersion: number;
  id: Uuid;
  name: string;
  assets: Asset[];
  sequence: Sequence;
}

export interface HistoryState {
  canUndo: boolean;
  canRedo: boolean;
  undoLabel: string | null;
  redoLabel: string | null;
  revision: number;
  dirty: boolean;
}

export interface ProjectState {
  project: Project;
  history: HistoryState;
  path: string | null;
  offline: Uuid[];
}

export type TrimEdge = "start" | "end";

export type EditCommand =
  | {
      type: "addClip";
      assetId: Uuid;
      trackId: Uuid;
      start: number;
      sourceIn?: Rational | null;
      duration?: number | null;
      includeLinkedAudio?: boolean;
    }
  | { type: "moveClip"; clipId: Uuid; trackId: Uuid | null; start: number }
  | { type: "trimClip"; clipId: Uuid; edge: TrimEdge; frame: number }
  | { type: "split"; frame: number; clipIds: Uuid[] }
  | { type: "deleteClips"; clipIds: Uuid[]; ripple: boolean }
  | { type: "setClipParams"; clipId: Uuid; params: ClipParams }
  | { type: "setTrackState"; trackId: Uuid; muted?: boolean; locked?: boolean; solo?: boolean }
  | { type: "addTrack"; kind: TrackKind };

export interface EditOutcome {
  label: string;
  created: Uuid[];
}

export interface EngineStatus {
  ok: boolean;
  version: string | null;
  ffmpegPath: string | null;
  h264Encoder: string | null;
  error: string | null;
}

export type ExportQuality = "high" | "standard" | "draft";

export interface ExportSettings {
  quality: ExportQuality;
  scale: number;
  audioBitrateKbps: number;
}

export interface VerifyCheck {
  name: string;
  expected: string;
  actual: string;
  pass: boolean;
}

export type ExportEvent =
  | { type: "progress"; fraction: number; outSeconds: number; totalSeconds: number; speed: string | null }
  | { type: "verifying" }
  | { type: "done"; path: string; encoder: string; elapsedSeconds: number; report: { pass: boolean; checks: VerifyCheck[] } }
  | { type: "failed"; message: string; log: string }
  | { type: "cancelled" };

export interface RecoveryInfo {
  projectName: string;
  originalPath: string | null;
  modified: number | null;
}

export const DEFAULT_PARAMS: ClipParams = { opacity: 1, scale: 1, positionX: 0, positionY: 0, gainDb: 0 };
