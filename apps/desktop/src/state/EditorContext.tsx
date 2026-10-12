import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { api } from "../api";
import { frameIndex, frameStartUs, sequenceDurationUs } from "../lib/time";
import type {
  Asset,
  Command,
  EngineReport,
  RecoveryInfo,
  RenderDone,
  Track,
  UiState,
} from "../types";

type RenderUi = {
  active: boolean;
  ratio: number;
  mode: string | null;
  message: string;
};

export type EditorController = {
  state: UiState;
  engine: EngineReport | null;
  error: string | null;
  clearError: () => void;
  selectedClipId: string | null;
  selectClip: (id: string | null) => void;
  playheadUs: number;
  playing: boolean;
  snap: boolean;
  setSnap: (value: boolean) => void;
  pps: number;
  setPps: (value: number | ((current: number) => number)) => void;
  render: RenderUi;
  preview: { path: string; revision: number } | null;
  recovery: RecoveryInfo | null;
  videoRef: RefObject<HTMLVideoElement>;
  exportOpen: boolean;
  setExportOpen: (open: boolean) => void;
  exportReport: RenderDone | null;
  dispatch: (command: Command) => Promise<void>;
  importMedia: () => Promise<void>;
  placeAsset: (asset: Asset, track: Track, timelineStartUs: number) => Promise<void>;
  seek: (timeUs: number) => void;
  notePlayhead: (timeUs: number) => void;
  notePlaying: (playing: boolean) => void;
  togglePlay: () => Promise<void>;
  step: (frames: number) => void;
  splitSelected: () => Promise<void>;
  deleteSelected: () => Promise<void>;
  unlinkSelected: () => Promise<void>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  newProject: () => Promise<void>;
  openProject: () => Promise<void>;
  saveProject: (saveAs?: boolean) => Promise<void>;
  loadSample: () => Promise<void>;
  restoreRecovery: () => Promise<void>;
  discardRecovery: () => Promise<void>;
  startExport: (request: {
    output: string;
    width: number;
    height: number;
    crf: number;
    preset: string;
  }) => Promise<void>;
  cancelRender: () => Promise<void>;
  onVideoReady: () => void;
  quit: () => Promise<void>;
  resetLayoutToken: number;
  resetLayout: () => void;
};

const EditorContext = createContext<EditorController | null>(null);

export function useEditor(): EditorController {
  const value = useContext(EditorContext);
  if (!value) throw new Error("Editor controls are only available inside the PulseCut shell.");
  return value;
}

export function EditorProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<UiState | null>(null);
  const [engine, setEngine] = useState<EngineReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedClipId, setSelectedClipId] = useState<string | null>(null);
  const [playheadUs, setPlayheadUs] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [snap, setSnap] = useState(true);
  const [pps, setPps] = useState(80);
  const [render, setRender] = useState<RenderUi>({ active: false, ratio: 0, mode: null, message: "" });
  const [preview, setPreview] = useState<{ path: string; revision: number } | null>(null);
  const [recovery, setRecovery] = useState<RecoveryInfo | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [exportReport, setExportReport] = useState<RenderDone | null>(null);
  const [resetLayoutToken, setResetLayoutToken] = useState(0);
  const videoRef = useRef<HTMLVideoElement>(null);
  const pendingPlay = useRef(false);
  const revisionRef = useRef(0);
  const stateRef = useRef<UiState | null>(null);
  const previewRef = useRef(preview);
  const playheadRef = useRef(0);

  useEffect(() => {
    revisionRef.current = state?.revision ?? 0;
    stateRef.current = state;
  }, [state]);

  const fail = useCallback((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    setError(message);
  }, []);

  const apply = useCallback((next: UiState) => {
    setState(next);
    setError(null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const unlisten: UnlistenFn[] = [];
    type UnlistenFn = () => void;
    void (async () => {
      try {
        const [editor, report, recovered] = await Promise.all([
          api.editorState(),
          api.engineReport(),
          api.recoveryInfo(),
        ]);
        if (cancelled) return;
        setState(editor);
        setEngine(report);
        setRecovery(recovered);
        if (!report.ok) setError(report.message);
        unlisten.push(
          await api.onRenderProgress((event) => {
            setRender({ active: true, ratio: event.ratio, mode: event.mode, message: event.message });
          }),
        );
        unlisten.push(
          await api.onRenderDone((event) => {
            setRender({ active: false, ratio: 1, mode: null, message: "" });
            if (event.mode === "preview") {
              setPreview({ path: event.output, revision: event.revision });
              if (pendingPlay.current && event.revision === revisionRef.current) {
                pendingPlay.current = true;
              } else {
                pendingPlay.current = false;
              }
            } else {
              setExportReport(event);
              setExportOpen(true);
            }
          }),
        );
        unlisten.push(
          await api.onRenderError((message) => {
            pendingPlay.current = false;
            setRender({ active: false, ratio: 0, mode: null, message: "" });
            setError(message);
          }),
        );
        unlisten.push(
          await api.onRenderCancelled(() => {
            pendingPlay.current = false;
            setRender({ active: false, ratio: 0, mode: null, message: "" });
            setError("Render cancelled.");
          }),
        );
      } catch (err) {
        if (!cancelled) fail(err);
      }
    })();
    return () => {
      cancelled = true;
      unlisten.forEach((stop) => stop());
    };
  }, [fail]);

  useEffect(() => {
    if (!state || !selectedClipId) return;
    const exists = state.project.sequence.tracks.some((track) =>
      track.clips.some((clip) => clip.id === selectedClipId),
    );
    if (!exists) setSelectedClipId(null);
  }, [state, selectedClipId]);

  const dispatch = useCallback(
    async (command: Command) => {
      try {
        apply(await api.dispatch(command));
      } catch (err) {
        fail(err);
      }
    },
    [apply, fail],
  );

  const importPaths = useCallback(
    async (paths: string[]) => {
      for (const path of paths) {
        try {
          const asset = await api.probeMedia(path);
          apply(await api.dispatch({ type: "importAsset", asset }));
        } catch (err) {
          fail(err);
        }
      }
    },
    [apply, fail],
  );

  const importMedia = useCallback(async () => {
    try {
      const paths = await api.pickMedia();
      await importPaths(paths);
    } catch (err) {
      fail(err);
    }
  }, [fail, importPaths]);

  const placeAsset = useCallback(
    async (asset: Asset, track: Track, timelineStartUs: number) => {
      if (asset.offline) {
        setError(`"${asset.name}" is offline. Relink it before placing it on the timeline.`);
        return;
      }
      const audioTrack = state?.project.sequence.tracks.find(
        (item) => item.kind === "audio" && !item.locked,
      );
      const videoTrackId = track.kind === "video" ? track.id : null;
      const audioTrackId =
        track.kind === "audio" ? track.id : asset.hasAudio && track.kind === "video" ? audioTrack?.id ?? null : null;
      if (track.kind === "video" && !asset.hasVideo) {
        setError(`"${asset.name}" has no video stream.`);
        return;
      }
      if (track.kind === "audio" && !asset.hasAudio) {
        setError(`"${asset.name}" has no audio stream.`);
        return;
      }
      await dispatch({
        type: "placeAsset",
        assetId: asset.id,
        videoTrackId,
        audioTrackId,
        timelineStartUs: Math.max(0, Math.round(timelineStartUs)),
        sourceInUs: 0,
        durationUs: null,
      });
    },
    [dispatch, state],
  );

  const notePlayhead = useCallback((timeUs: number) => {
    setPlayheadUs(Math.max(0, timeUs));
  }, []);

  const notePlaying = useCallback((value: boolean) => {
    setPlaying(value);
  }, []);

  const seek = useCallback((timeUs: number) => {
    const next = Math.max(0, timeUs);
    setPlayheadUs(next);
    const current = stateRef.current;
    const video = videoRef.current;
    if (video && current && previewMatches(previewRef.current, current.revision)) {
      const target = next / 1_000_000;
      if (Math.abs(video.currentTime - target) > 0.0005) {
        video.currentTime = target;
      }
    }
  }, []);

  useEffect(() => {
    previewRef.current = preview;
  }, [preview]);

  const step = useCallback(
    (frames: number) => {
      const current = stateRef.current;
      if (!current) return;
      const { fpsNum, fpsDen } = current.project.sequence;
      const index = frameIndex(playheadRef.current, fpsNum, fpsDen) + frames;
      seek(frameStartUs(Math.max(0, index), fpsNum, fpsDen));
    },
    [seek],
  );

  useEffect(() => {
    playheadRef.current = playheadUs;
  }, [playheadUs]);

  const togglePlay = useCallback(async () => {
    const current = stateRef.current;
    if (!current) return;
    const video = videoRef.current;
    if (playing && video) {
      video.pause();
      setPlaying(false);
      return;
    }
    if (sequenceDurationUs(current.project.sequence.tracks) <= 0) {
      setError("Add a clip to the timeline before playing.");
      return;
    }
    if (!engine?.ok) {
      setError(engine?.message ?? "FFmpeg is not available, so preview cannot play.");
      return;
    }
    if (!preview || preview.revision !== current.revision) {
      pendingPlay.current = true;
      setRender({ active: true, ratio: 0, mode: "preview", message: "Rendering preview…" });
      try {
        await api.startRender({ mode: "preview" });
      } catch (err) {
        pendingPlay.current = false;
        setRender({ active: false, ratio: 0, mode: null, message: "" });
        fail(err);
      }
      return;
    }
    try {
      await video?.play();
      setPlaying(true);
    } catch (err) {
      fail(err);
    }
  }, [engine, fail, playing, preview]);

  const onVideoReady = useCallback(() => {
    if (!pendingPlay.current) return;
    pendingPlay.current = false;
    const current = stateRef.current;
    if (!current || !previewMatches(previewRef.current, current.revision)) return;
    void videoRef.current?.play().then(
      () => setPlaying(true),
      (err: unknown) => fail(err),
    );
  }, [fail]);

  const selectedClip = useMemo(() => {
    if (!state || !selectedClipId) return null;
    for (const track of state.project.sequence.tracks) {
      const clip = track.clips.find((item) => item.id === selectedClipId);
      if (clip) return clip;
    }
    return null;
  }, [selectedClipId, state]);

  const splitSelected = useCallback(async () => {
    if (!selectedClip) {
      setError("Select a clip before splitting.");
      return;
    }
    await dispatch({ type: "splitClip", clipId: selectedClip.id, timelineUs: Math.round(playheadRef.current) });
  }, [dispatch, selectedClip]);

  const deleteSelected = useCallback(async () => {
    if (!selectedClip) {
      setError("Select a clip before deleting.");
      return;
    }
    await dispatch({ type: "deleteClip", clipId: selectedClip.id });
  }, [dispatch, selectedClip]);

  const unlinkSelected = useCallback(async () => {
    if (!selectedClip) {
      setError("Select a linked clip first.");
      return;
    }
    await dispatch({ type: "unlinkClip", clipId: selectedClip.id });
  }, [dispatch, selectedClip]);

  const undo = useCallback(async () => {
    await dispatch({ type: "undo" });
  }, [dispatch]);
  const redo = useCallback(async () => {
    await dispatch({ type: "redo" });
  }, [dispatch]);

  const newProject = useCallback(async () => {
    if (!stateRef.current) return;
    if (!(await api.confirmDiscard(stateRef.current.dirty))) return;
    try {
      apply(await api.newProject());
      setPreview(null);
      setPlayheadUs(0);
      setSelectedClipId(null);
      setRecovery(await api.recoveryInfo());
    } catch (err) {
      fail(err);
    }
  }, [apply, fail]);

  const openProject = useCallback(async () => {
    if (!stateRef.current) return;
    if (!(await api.confirmDiscard(stateRef.current.dirty))) return;
    try {
      const picked = await api.pickProject();
      if (!picked || Array.isArray(picked)) return;
      apply(await api.openProject(picked));
      setPreview(null);
      setPlayheadUs(0);
      setSelectedClipId(null);
      setRecovery(await api.recoveryInfo());
    } catch (err) {
      fail(err);
    }
  }, [apply, fail]);

  const saveProject = useCallback(
    async (saveAs = false) => {
      const current = stateRef.current;
      if (!current) return;
      try {
        let path = saveAs ? null : current.path;
        if (!path) {
          const suggested = `${current.project.name || "Untitled"}.pulsecut`;
          const picked = await api.pickProjectSave(suggested);
          if (!picked) return;
          path = picked;
        }
        apply(await api.saveProject(path));
        setRecovery(await api.recoveryInfo());
      } catch (err) {
        fail(err);
      }
    },
    [apply, fail],
  );

  const loadSample = useCallback(async () => {
    if (!stateRef.current) return;
    if (!(await api.confirmDiscard(stateRef.current.dirty))) return;
    try {
      apply(await api.loadSample());
      setPreview(null);
      setPlayheadUs(0);
      setSelectedClipId(null);
    } catch (err) {
      fail(err);
    }
  }, [apply, fail]);

  const restoreRecovery = useCallback(async () => {
    try {
      apply(await api.restoreRecovery());
      setPreview(null);
      setRecovery(await api.recoveryInfo());
    } catch (err) {
      fail(err);
    }
  }, [apply, fail]);

  const discardRecovery = useCallback(async () => {
    try {
      await api.discardRecovery();
      setRecovery(await api.recoveryInfo());
    } catch (err) {
      fail(err);
    }
  }, [fail]);

  const startExport = useCallback(
    async (request: { output: string; width: number; height: number; crf: number; preset: string }) => {
      if (!engine?.ok) {
        setError(engine?.message ?? "FFmpeg is not available.");
        return;
      }
      setExportReport(null);
      setRender({ active: true, ratio: 0, mode: "export", message: "Exporting…" });
      try {
        await api.startRender({ mode: "export", ...request });
      } catch (err) {
        setRender({ active: false, ratio: 0, mode: null, message: "" });
        fail(err);
      }
    },
    [engine, fail],
  );

  const cancelRender = useCallback(async () => {
    try {
      await api.cancelRender();
    } catch (err) {
      fail(err);
    }
  }, [fail]);

  const quit = useCallback(async () => {
    if (!stateRef.current) return;
    if (!(await api.confirmDiscard(stateRef.current.dirty))) return;
    await getCurrentWindow().close();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT")) {
        return;
      }
      const meta = event.metaKey || event.ctrlKey;
      if (event.code === "Space") {
        event.preventDefault();
        void togglePlay();
      } else if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
        event.preventDefault();
        const sequence = stateRef.current?.project.sequence;
        const nominal = sequence ? Math.max(1, Math.round(sequence.fpsNum / sequence.fpsDen)) : 30;
        const direction = event.key === "ArrowLeft" ? -1 : 1;
        step(direction * (event.shiftKey ? nominal : 1));
      } else if (event.key === "Home") {
        seek(0);
      } else if (event.key === "End") {
        const current = stateRef.current;
        if (current) seek(sequenceDurationUs(current.project.sequence.tracks));
      } else if (!meta && event.key.toLowerCase() === "s") {
        void splitSelected();
      } else if (!meta && event.key.toLowerCase() === "n") {
        setSnap((value) => !value);
      } else if (event.key === "Delete" || event.key === "Backspace") {
        event.preventDefault();
        void deleteSelected();
      } else if (meta && event.key.toLowerCase() === "z" && !event.shiftKey) {
        event.preventDefault();
        void undo();
      } else if (meta && (event.key.toLowerCase() === "y" || (event.shiftKey && event.key.toLowerCase() === "z"))) {
        event.preventDefault();
        void redo();
      } else if (meta && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void saveProject(false);
      } else if (meta && event.key.toLowerCase() === "i") {
        event.preventDefault();
        void importMedia();
      } else if (meta && event.key.toLowerCase() === "e") {
        event.preventDefault();
        setExportOpen(true);
      } else if (meta && (event.key === "=" || event.key === "+")) {
        event.preventDefault();
        setPps((value) => Math.min(400, value + 20));
      } else if (meta && event.key === "-") {
        event.preventDefault();
        setPps((value) => Math.max(20, value - 20));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [deleteSelected, importMedia, redo, saveProject, seek, splitSelected, step, togglePlay, undo]);

  if (!state) {
    return <div className="grid h-screen place-items-center bg-bg0 text-sm text-muted">Opening PulseCut…</div>;
  }

  const value: EditorController = {
    state,
    engine,
    error,
    clearError: () => setError(null),
    selectedClipId,
    selectClip: setSelectedClipId,
    playheadUs,
    playing,
    snap,
    setSnap,
    pps,
    setPps,
    render,
    preview,
    recovery,
    videoRef,
    exportOpen,
    setExportOpen,
    exportReport,
    dispatch,
    importMedia,
    placeAsset,
    seek,
    notePlayhead,
    notePlaying,
    togglePlay,
    step,
    splitSelected,
    deleteSelected,
    unlinkSelected,
    undo,
    redo,
    newProject,
    openProject,
    saveProject,
    loadSample,
    restoreRecovery,
    discardRecovery,
    startExport,
    cancelRender,
    onVideoReady,
    quit,
    resetLayoutToken,
    resetLayout: () => {
      localStorage.removeItem("pulsecut.layout.v1");
      setResetLayoutToken((token) => token + 1);
    },
  };

  return <EditorContext.Provider value={value}>{children}</EditorContext.Provider>;
}

function previewMatches(preview: { path: string; revision: number } | null, revision: number) {
  return Boolean(preview && preview.revision === revision);
}
