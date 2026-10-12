import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ACTIONS } from "../lib/actions";
import { useThumbnail, useWaveform } from "../lib/mediaCache";
import { snapPoint, snapRange, snapTargets } from "../lib/snap";
import { sequenceEnd, useStore } from "../lib/store";
import { formatTimecode, rulerStep, secondsToFrames, toNumber } from "../lib/time";
import type { Asset, Clip, Sequence, Track, Uuid } from "../lib/types";
import {
  IconEye,
  IconEyeOff,
  IconLink,
  IconLock,
  IconMagnet,
  IconMinus,
  IconMute,
  IconPlus,
  IconRazor,
  IconSpeaker,
  IconTrash,
  IconUnlock,
} from "./Icons";
import { ASSET_MIME } from "./MediaBrowser";
import { IconButton } from "./ui";

const HEADER_W = 172;
const RULER_H = 26;
const SNAP_PX = 8;
const HEIGHTS_KEY = "pulsecut.trackHeights.v1";

type DragMode = "move" | "trimStart" | "trimEnd";
interface Drag {
  clipId: Uuid;
  mode: DragMode;
  originX: number;
  started: boolean;
  delta: number;
  trackId: Uuid;
  snappedTo: number | null;
  group: Set<Uuid>;
}

function linkedIds(seq: Sequence, clip: Clip): Set<Uuid> {
  const ids = new Set<Uuid>([clip.id]);
  if (clip.link) for (const t of seq.tracks) for (const c of t.clips) if (c.link === clip.link) ids.add(c.id);
  return ids;
}

function Waveform({ asset, clip, width, height, fps }: { asset: Asset; clip: Clip; width: number; height: number; fps: number }) {
  const wf = useWaveform(asset.id, !!asset.media.audio);
  const ref = useRef<HTMLCanvasElement>(null);
  const w = Math.max(1, Math.min(Math.round(width), 6000));
  useEffect(() => {
    const c = ref.current;
    if (!c || !wf) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.round(w * dpr);
    c.height = Math.round(height * dpr);
    const ctx = c.getContext("2d")!;
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.fillStyle = "rgba(170, 240, 220, 0.75)";
    const startPeak = toNumber(clip.sourceIn) * wf.peaksPerSecond;
    const peaksInClip = (clip.duration / fps) * wf.peaksPerSecond;
    const gain = Math.pow(10, clip.params.gainDb / 20);
    const mid = c.height / 2;
    for (let x = 0; x < c.width; x++) {
      const a = Math.floor(startPeak + (x / c.width) * peaksInClip);
      const b = Math.max(a + 1, Math.floor(startPeak + ((x + 1) / c.width) * peaksInClip));
      let m = 0;
      for (let i = a; i < b && i < wf.peaks.length; i++) m = Math.max(m, wf.peaks[i]);
      const h = Math.min(1, m * gain) * mid * 0.95;
      ctx.fillRect(x, mid - h, 1, Math.max(1, h * 2));
    }
  }, [wf, w, height, clip.sourceIn, clip.duration, clip.params.gainDb, fps]);
  return <canvas ref={ref} className="pointer-events-none absolute inset-x-0 bottom-0" style={{ width: Math.min(width, 6000), height }} />;
}

function ClipView({
  clip,
  track,
  asset,
  ppf,
  fps,
  selected,
  offline,
  ghost,
  dimmed,
  onPointerDown,
}: {
  clip: Clip;
  track: Track;
  asset: Asset | undefined;
  ppf: number;
  fps: number;
  selected: boolean;
  offline: boolean;
  ghost?: boolean;
  dimmed?: boolean;
  onPointerDown?: (e: React.PointerEvent, mode: DragMode) => void;
}) {
  const isVideo = track.kind === "video";
  const thumb = useThumbnail(asset?.id ?? null, isVideo && !!asset?.media.video && !offline);
  const left = clip.start * ppf;
  const width = Math.max(2, clip.duration * ppf);
  const base = offline ? "bg-pc-danger/35 border-pc-danger/70" : isVideo ? "bg-pc-clip-video border-pc-clip-video-hi" : "bg-pc-clip-audio border-pc-clip-audio-hi";
  return (
    <div
      data-clip-id={clip.id}
      className={`group absolute top-[3px] bottom-[3px] overflow-hidden rounded-[3px] border ${base} ${selected ? "ring-2 ring-pc-accent-hi" : ""} ${
        ghost ? "pointer-events-none opacity-80 shadow-lg" : ""
      } ${dimmed ? "opacity-35" : ""} ${track.locked ? "cursor-not-allowed" : "cursor-grab active:cursor-grabbing"}`}
      style={{ left, width }}
      onPointerDown={(e) => onPointerDown?.(e, "move")}
      title={`${asset?.name ?? "?"}\nStart ${formatTimecode(clip.start, { num: fps, den: 1 })} · ${clip.duration} frames`}
    >
      {isVideo && thumb && (
        <div className="pointer-events-none absolute inset-0 opacity-60" style={{ backgroundImage: `url(${thumb})`, backgroundSize: "auto 100%", backgroundRepeat: "repeat-x" }} />
      )}
      {!isVideo && asset && !offline && <Waveform asset={asset} clip={clip} width={width} height={Math.max(8, 40)} fps={fps} />}
      <div className="pointer-events-none relative flex items-center gap-1 truncate bg-gradient-to-b from-black/55 to-transparent px-1.5 pt-0.5 text-[11px] text-white/95">
        {clip.link && <IconLink size={10} />}
        <span className="truncate">{offline ? `OFFLINE · ${asset?.name ?? ""}` : asset?.name}</span>
        {clip.params.opacity < 1 && isVideo && <span className="text-white/60">{Math.round(clip.params.opacity * 100)}%</span>}
        {clip.params.gainDb !== 0 && !isVideo && <span className="text-white/60">{clip.params.gainDb > 0 ? "+" : ""}{clip.params.gainDb.toFixed(1)} dB</span>}
      </div>
      {!track.locked && !ghost && (
        <>
          <div
            className="absolute inset-y-0 left-0 w-[6px] cursor-ew-resize bg-white/0 hover:bg-pc-accent-hi/70"
            onPointerDown={(e) => {
              e.stopPropagation();
              onPointerDown?.(e, "trimStart");
            }}
            aria-hidden
          />
          <div
            className="absolute inset-y-0 right-0 w-[6px] cursor-ew-resize bg-white/0 hover:bg-pc-accent-hi/70"
            onPointerDown={(e) => {
              e.stopPropagation();
              onPointerDown?.(e, "trimEnd");
            }}
            aria-hidden
          />
        </>
      )}
    </div>
  );
}

function Ruler({ seq, ppf, width, onScrub }: { seq: Sequence; ppf: number; width: number; onScrub: (clientX: number, rect: DOMRect) => void }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const fps = seq.frameRate;
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.min(width, 32000);
    c.width = Math.round(w * dpr);
    c.height = Math.round(RULER_H * dpr);
    const ctx = c.getContext("2d")!;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, w, RULER_H);
    const { major, minor } = rulerStep(ppf, fps);
    ctx.strokeStyle = "#3a3f47";
    ctx.fillStyle = "#9aa3ad";
    ctx.font = "10px ui-monospace, Consolas, monospace";
    const frames = Math.ceil(w / ppf);
    ctx.beginPath();
    for (let f = 0; f <= frames; f += minor) {
      const x = Math.round(f * ppf) + 0.5;
      const isMajor = f % major === 0;
      ctx.moveTo(x, isMajor ? 8 : 17);
      ctx.lineTo(x, RULER_H);
      if (isMajor) ctx.fillText(formatTimecode(f, fps), x + 3, 10);
    }
    ctx.stroke();
  }, [ppf, width, fps]);
  return (
    <canvas
      ref={ref}
      className="block cursor-text"
      style={{ width: Math.min(width, 32000), height: RULER_H }}
      aria-label="Timeline ruler: click or drag to move the playhead"
      onPointerDown={(e) => {
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        onScrub(e.clientX, (e.target as HTMLElement).getBoundingClientRect());
      }}
      onPointerMove={(e) => {
        if ((e.target as HTMLElement).hasPointerCapture(e.pointerId)) onScrub(e.clientX, (e.target as HTMLElement).getBoundingClientRect());
      }}
    />
  );
}

export function Timeline() {
  const ps = useStore((s) => s.ps);
  const playhead = useStore((s) => s.playhead);
  const playing = useStore((s) => s.playing);
  const ppf = useStore((s) => s.pxPerFrame);
  const snap = useStore((s) => s.snap);
  const selection = useStore((s) => s.selection);
  const { setPlayhead, setSelection, edit, setZoom, toggleSnap, stopPlayback } = useStore.getState();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [drop, setDrop] = useState<{ trackId: Uuid; frame: number; length: number } | null>(null);
  const [heights, setHeights] = useState<Record<string, number>>(() => {
    try {
      return JSON.parse(localStorage.getItem(HEIGHTS_KEY) ?? "{}");
    } catch {
      return {};
    }
  });
  const dragRef = useRef<Drag | null>(null);
  dragRef.current = drag;

  const seq = ps?.project.sequence;
  const fpsNum = seq ? seq.frameRate.num / seq.frameRate.den : 30;
  const end = sequenceEnd(ps);
  const contentFrames = Math.max(end + Math.round(fpsNum * 30), Math.round(fpsNum * 60));
  const laneWidth = contentFrames * ppf;
  const offline = useMemo(() => new Set(ps?.offline ?? []), [ps?.offline]);
  const assets = useMemo(() => new Map((ps?.project.assets ?? []).map((a) => [a.id, a])), [ps?.project.assets]);
  const selected = useMemo(() => {
    const set = new Set(selection);
    if (seq) for (const t of seq.tracks) for (const c of t.clips) if (set.has(c.id)) for (const id of linkedIds(seq, c)) set.add(id);
    return set;
  }, [selection, seq]);

  const ordered = useMemo(() => {
    if (!seq) return [];
    const v = seq.tracks.filter((t) => t.kind === "video").reverse();
    const a = seq.tracks.filter((t) => t.kind === "audio");
    return [...v, ...a];
  }, [seq]);
  const trackHeight = (t: Track) => heights[t.id] ?? (t.kind === "video" ? 58 : 50);

  const scrub = useCallback(
    (clientX: number, rect: DOMRect) => {
      if (useStore.getState().playing) void stopPlayback();
      const f = Math.round((clientX - rect.left) / ppf);
      setPlayhead(Math.max(0, Math.min(f, end)));
    },
    [ppf, end, setPlayhead, stopPlayback],
  );

  // Zoom-to-fit and ctrl+wheel zoom anchored at the cursor.
  useEffect(() => {
    const fit = () => {
      const el = scrollRef.current;
      if (!el) return;
      const frames = Math.max(end, Math.round(fpsNum * 10));
      setZoom((el.clientWidth - HEADER_W - 40) / frames);
      el.scrollLeft = 0;
    };
    window.addEventListener("pulsecut:zoom-fit", fit);
    return () => window.removeEventListener("pulsecut:zoom-fit", fit);
  }, [end, fpsNum, setZoom]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      e.preventDefault();
      const rect = el.getBoundingClientRect();
      const x = e.clientX - rect.left - HEADER_W + el.scrollLeft;
      const frameAtCursor = x / ppf;
      const next = Math.min(40, Math.max(0.02, ppf * (e.deltaY < 0 ? 1.2 : 1 / 1.2)));
      setZoom(next);
      requestAnimationFrame(() => {
        el.scrollLeft = frameAtCursor * next - (e.clientX - rect.left - HEADER_W);
      });
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [ppf, setZoom]);

  // Keep the playhead in view while playing.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || !playing) return;
    const x = playhead * ppf;
    const viewW = el.clientWidth - HEADER_W;
    if (x < el.scrollLeft || x > el.scrollLeft + viewW - 40) el.scrollLeft = Math.max(0, x - 40);
  }, [playhead, playing, ppf]);

  const startDrag = (e: React.PointerEvent, clip: Clip, track: Track, mode: DragMode) => {
    if (e.button !== 0 || !seq) return;
    e.preventDefault();
    const multi = e.shiftKey || e.ctrlKey || e.metaKey;
    if (multi) setSelection(selection.includes(clip.id) ? selection.filter((id) => id !== clip.id) : [...selection, clip.id]);
    else if (!selected.has(clip.id)) setSelection([clip.id]);
    if (track.locked || multi) return;
    setDrag({ clipId: clip.id, mode, originX: e.clientX, started: false, delta: 0, trackId: track.id, snappedTo: null, group: linkedIds(seq, clip) });
  };

  useEffect(() => {
    if (!drag || !seq) return;
    const located = (() => {
      for (const t of seq.tracks) {
        const c = t.clips.find((x) => x.id === drag.clipId);
        if (c) return { clip: c, track: t };
      }
      return null;
    })();
    if (!located) return;
    const { clip, track } = located;
    const targets = snapTargets(seq, playhead, drag.group);
    const onMove = (e: PointerEvent) => {
      const d = dragRef.current!;
      const dx = e.clientX - d.originX;
      if (!d.started && Math.abs(dx) < 3) return;
      let delta = Math.round(dx / ppf);
      let snappedTo: number | null = null;
      let trackId = d.trackId;
      const thr = SNAP_PX / ppf;
      if (d.mode === "move") {
        let start = Math.max(0, clip.start + delta);
        if (snap) ({ start, snappedTo } = snapRange(start, clip.duration, targets, thr));
        delta = Math.max(0, start) - clip.start;
        const el = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-track-id]");
        const t = el && seq.tracks.find((x) => x.id === el.dataset.trackId);
        if (t && t.kind === track.kind && !t.locked) trackId = t.id;
      } else {
        const edge = d.mode === "trimStart" ? clip.start : clip.start + clip.duration;
        let f = edge + delta;
        if (snap) ({ frame: f, snappedTo } = snapPoint(f, targets, thr));
        delta = f - edge;
        if (d.mode === "trimStart") delta = Math.min(delta, clip.duration - 1);
        else delta = Math.max(delta, -(clip.duration - 1));
      }
      setDrag({ ...d, started: true, delta, trackId, snappedTo });
    };
    const onUp = () => {
      const d = dragRef.current;
      setDrag(null);
      if (!d || !d.started) return;
      if (d.mode === "move" && (d.delta !== 0 || d.trackId !== track.id))
        void edit({ type: "moveClip", clipId: clip.id, trackId: d.trackId === track.id ? null : d.trackId, start: clip.start + d.delta });
      else if (d.mode === "trimStart" && d.delta !== 0) void edit({ type: "trimClip", clipId: clip.id, edge: "start", frame: clip.start + d.delta });
      else if (d.mode === "trimEnd" && d.delta !== 0) void edit({ type: "trimClip", clipId: clip.id, edge: "end", frame: clip.start + clip.duration + d.delta });
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drag?.clipId, drag?.mode, seq, ppf, snap, playhead]);

  if (!seq || !ps) return <section className="h-full bg-pc-panel" />;

  const ghostFor = (c: Clip, t: Track): Clip | null => {
    if (!drag?.started || !drag.group.has(c.id)) return null;
    if (drag.mode === "move") return { ...c, start: c.start + drag.delta };
    if (drag.mode === "trimStart") return { ...c, start: c.start + drag.delta, duration: c.duration - drag.delta };
    void t;
    return { ...c, duration: c.duration + drag.delta };
  };

  const onLaneDragOver = (e: React.DragEvent, track: Track) => {
    if (!e.dataTransfer.types.includes(ASSET_MIME) || track.locked) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    let frame = Math.max(0, Math.round((e.clientX - rect.left) / ppf));
    const assetId = useStore.getState().selectedAsset;
    const asset = assetId ? assets.get(assetId) : undefined;
    const length = asset && asset.media.kind !== "image" ? secondsToFrames(toNumber(asset.media.duration), seq.frameRate) : Math.round(fpsNum * 5);
    if (snap) frame = Math.max(0, snapRange(frame, length, snapTargets(seq, playhead, new Set()), SNAP_PX / ppf).start);
    setDrop({ trackId: track.id, frame, length });
  };

  const tc = (f: number) => formatTimecode(f, seq.frameRate);

  return (
    <section className="flex h-full min-h-0 flex-col bg-pc-panel" aria-label="Timeline">
      <div className="flex h-9 shrink-0 items-center gap-1 border-b border-pc-border bg-pc-panel-2 px-2">
        <span className="mr-2 text-[11px] font-semibold tracking-wider text-pc-muted uppercase">Timeline</span>
        <IconButton label={`Snapping ${snap ? "on" : "off"} (N)`} active={snap} onClick={toggleSnap}>
          <IconMagnet />
        </IconButton>
        <IconButton label="Split at playhead (S)" disabled={end === 0} onClick={() => ACTIONS["tl.split"].run()}>
          <IconRazor />
        </IconButton>
        <IconButton label="Delete selected (Del) — Shift+Del ripples" disabled={selection.length === 0} onClick={() => ACTIONS["edit.delete"].run()}>
          <IconTrash />
        </IconButton>
        <div className="mx-1 h-5 w-px bg-pc-border" />
        <IconButton label="Add video track" onClick={() => ACTIONS["tl.addVideo"].run()}>
          <span className="text-[10.5px] font-semibold">+V</span>
        </IconButton>
        <IconButton label="Add audio track" onClick={() => ACTIONS["tl.addAudio"].run()}>
          <span className="text-[10.5px] font-semibold">+A</span>
        </IconButton>
        <div className="flex-1" />
        <span className="tabular mr-2 text-[11px] text-pc-faint">
          {drag?.started
            ? drag.mode === "move"
              ? `Δ ${drag.delta >= 0 ? "+" : ""}${drag.delta} f`
              : `trim ${drag.delta >= 0 ? "+" : ""}${drag.delta} f`
            : `${tc(playhead)} / ${tc(end)}`}
        </span>
        <IconButton label="Zoom out (-)" onClick={() => setZoom(ppf / 1.4)}>
          <IconMinus />
        </IconButton>
        <input
          type="range"
          aria-label="Timeline zoom"
          min={-4}
          max={3.7}
          step={0.01}
          value={Math.log(ppf)}
          onChange={(e) => setZoom(Math.exp(Number(e.target.value)))}
          className="h-1 w-28 accent-[var(--color-pc-accent)]"
        />
        <IconButton label="Zoom in (=)" onClick={() => setZoom(ppf * 1.4)}>
          <IconPlus />
        </IconButton>
        <IconButton label="Zoom to fit (Shift+Z)" onClick={() => ACTIONS["view.zoomFit"].run()}>
          <span className="text-[10.5px] font-semibold">FIT</span>
        </IconButton>
      </div>

      <div ref={scrollRef} className="relative min-h-0 flex-1 overflow-auto" onDragLeave={() => setDrop(null)}>
        <div className="relative" style={{ width: HEADER_W + laneWidth }}>
          <div className="sticky top-0 z-30 flex border-b border-pc-border bg-pc-panel-2" style={{ height: RULER_H }}>
            <div className="sticky left-0 z-40 shrink-0 border-r border-pc-border bg-pc-panel-2" style={{ width: HEADER_W }} />
            <Ruler seq={seq} ppf={ppf} width={laneWidth} onScrub={scrub} />
          </div>

          {ordered.map((track, i) => {
            const h = trackHeight(track);
            const firstAudio = track.kind === "audio" && ordered[i - 1]?.kind === "video";
            return (
              <div key={track.id} className={`flex ${firstAudio ? "border-t-2 border-t-pc-border-strong" : ""}`} style={{ height: h }}>
                <div className="sticky left-0 z-20 flex shrink-0 flex-col justify-center border-r border-b border-pc-border bg-pc-panel-2 px-2" style={{ width: HEADER_W }}>
                  <div className="flex items-center gap-1">
                    <span className={`w-7 text-[11.5px] font-semibold ${track.kind === "video" ? "text-pc-accent-hi" : "text-pc-clip-audio-hi"}`}>{track.name}</span>
                    {track.kind === "video" ? (
                      <IconButton label={track.muted ? `Show ${track.name}` : `Hide ${track.name} from output`} active={track.muted} onClick={() => edit({ type: "setTrackState", trackId: track.id, muted: !track.muted })}>
                        {track.muted ? <IconEyeOff size={14} /> : <IconEye size={14} />}
                      </IconButton>
                    ) : (
                      <>
                        <IconButton label={track.muted ? `Unmute ${track.name}` : `Mute ${track.name}`} active={track.muted} onClick={() => edit({ type: "setTrackState", trackId: track.id, muted: !track.muted })}>
                          {track.muted ? <IconMute size={14} /> : <IconSpeaker size={14} />}
                        </IconButton>
                        <IconButton label={track.solo ? `Unsolo ${track.name}` : `Solo ${track.name}`} active={track.solo} onClick={() => edit({ type: "setTrackState", trackId: track.id, solo: !track.solo })}>
                          <span className="text-[10.5px] font-bold">S</span>
                        </IconButton>
                      </>
                    )}
                    <IconButton label={track.locked ? `Unlock ${track.name}` : `Lock ${track.name}`} active={track.locked} onClick={() => edit({ type: "setTrackState", trackId: track.id, locked: !track.locked })}>
                      {track.locked ? <IconLock size={14} /> : <IconUnlock size={14} />}
                    </IconButton>
                  </div>
                  <div
                    role="separator"
                    aria-label={`Resize ${track.name}`}
                    className="absolute inset-x-0 bottom-0 h-[5px] cursor-row-resize hover:bg-pc-accent/40"
                    onPointerDown={(e) => {
                      (e.target as HTMLElement).setPointerCapture(e.pointerId);
                      const y0 = e.clientY;
                      const h0 = h;
                      const move = (ev: PointerEvent) => setHeights((s) => ({ ...s, [track.id]: Math.min(160, Math.max(30, h0 + ev.clientY - y0)) }));
                      const up = () => {
                        window.removeEventListener("pointermove", move);
                        setHeights((s) => (localStorage.setItem(HEIGHTS_KEY, JSON.stringify(s)), s));
                      };
                      window.addEventListener("pointermove", move);
                      window.addEventListener("pointerup", up, { once: true });
                    }}
                  />
                </div>
                <div
                  data-track-id={track.id}
                  className={`relative border-b border-pc-border ${track.locked ? "bg-[repeating-linear-gradient(135deg,#15171a_0_6px,#181a1e_6px_12px)]" : "bg-pc-bg"} ${
                    drag?.started && drag.mode === "move" && drag.trackId === track.id ? "bg-pc-accent/5" : ""
                  }`}
                  style={{ width: laneWidth }}
                  onPointerDown={(e) => {
                    if (e.target !== e.currentTarget) return;
                    setSelection([]);
                    scrub(e.clientX, (e.currentTarget as HTMLElement).getBoundingClientRect());
                  }}
                  onDragOver={(e) => onLaneDragOver(e, track)}
                  onDrop={(e) => {
                    const assetId = e.dataTransfer.getData(ASSET_MIME);
                    setDrop(null);
                    if (!assetId || !drop) return;
                    e.preventDefault();
                    void edit({ type: "addClip", assetId, trackId: track.id, start: drop.frame, includeLinkedAudio: true });
                  }}
                >
                  {track.clips.map((c) => {
                    const ghost = ghostFor(c, track);
                    const movedAway = ghost && drag?.mode === "move" && drag.clipId === c.id && drag.trackId !== track.id;
                    return (
                      <ClipView
                        key={c.id}
                        clip={ghost && !movedAway ? ghost : c}
                        track={track}
                        asset={assets.get(c.assetId)}
                        ppf={ppf}
                        fps={fpsNum}
                        selected={selected.has(c.id)}
                        offline={offline.has(c.assetId)}
                        ghost={!!ghost && !movedAway}
                        dimmed={!!movedAway}
                        onPointerDown={(e, mode) => startDrag(e, c, track, mode)}
                      />
                    );
                  })}
                  {drag?.started && drag.mode === "move" && drag.trackId === track.id && (() => {
                    const src = seq.tracks.flatMap((t) => t.clips.map((c) => ({ c, t }))).find((x) => x.c.id === drag.clipId);
                    if (!src || src.t.id === track.id) return null;
                    return <ClipView clip={{ ...src.c, start: src.c.start + drag.delta }} track={track} asset={assets.get(src.c.assetId)} ppf={ppf} fps={fpsNum} selected offline={false} ghost />;
                  })()}
                  {drop?.trackId === track.id && (
                    <div className="pointer-events-none absolute top-[3px] bottom-[3px] rounded-[3px] border-2 border-dashed border-pc-accent-hi bg-pc-accent/15" style={{ left: drop.frame * ppf, width: Math.max(4, drop.length * ppf) }} />
                  )}
                </div>
              </div>
            );
          })}

          <div className="pointer-events-none absolute top-0 bottom-0 z-[25]" style={{ left: HEADER_W + playhead * ppf }}>
            <div className="absolute top-0 h-[10px] w-[11px] -translate-x-1/2 bg-pc-accent [clip-path:polygon(0_0,100%_0,50%_100%)]" />
            <div className="absolute top-0 bottom-0 w-px bg-pc-accent" />
          </div>
          {drag?.snappedTo != null && (
            <div className="pointer-events-none absolute top-0 bottom-0 z-[26] w-px bg-pc-warn" style={{ left: HEADER_W + drag.snappedTo * ppf }} />
          )}
        </div>
        {ordered.length > 0 && end === 0 && (
          <div className="pointer-events-none absolute inset-x-0 bottom-3 text-center text-[12px] text-pc-faint">Drag media from the Project Media panel onto a track, or select it and press E.</div>
        )}
      </div>
    </section>
  );
}
