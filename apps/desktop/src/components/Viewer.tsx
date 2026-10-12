import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { ACTIONS } from "../lib/actions";
import { errorMessage, ipc } from "../lib/ipc";
import { playback } from "../lib/playback";
import { sequenceEnd, useStore } from "../lib/store";
import { formatTimecode } from "../lib/time";
import { IconPause, IconPlay, IconStepBack, IconStepFwd, IconToEnd, IconToStart } from "./Icons";
import { IconButton, PanelHeader } from "./ui";

type Zoom = "fit" | 0.5 | 1;

export function Viewer() {
  const ps = useStore((s) => s.ps);
  const playhead = useStore((s) => s.playhead);
  const playing = useStore((s) => s.playing);
  const dropped = useStore((s) => s.droppedFrames);
  const previewScale = useStore((s) => s.previewScale);
  const setPreviewScale = useStore((s) => s.setPreviewScale);
  const safeAreas = useStore((s) => s.safeAreas);
  const toggleSafe = useStore((s) => s.toggleSafeAreas);
  const engineOk = useStore((s) => !!s.engine?.ok);
  const [zoom, setZoom] = useState<Zoom>("fit");
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [renderError, setRenderError] = useState<string | null>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const lastBitmap = useRef<ImageBitmap | null>(null);

  const seq = ps?.project.sequence;
  const end = sequenceEnd(ps);
  const fps = seq?.frameRate ?? { num: 30, den: 1 };

  useLayoutEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const aspect = seq ? seq.width / seq.height : 16 / 9;
  let dw: number, dh: number;
  if (zoom === "fit" || !seq) {
    dw = Math.max(1, Math.min(box.w - 16, (box.h - 16) * aspect));
    dh = dw / aspect;
  } else {
    dw = (seq.width * zoom) / window.devicePixelRatio;
    dh = (seq.height * zoom) / window.devicePixelRatio;
  }

  const draw = (bmp: ImageBitmap | null) => {
    const c = canvasRef.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, c.width, c.height);
    if (bmp) {
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(bmp, 0, 0, c.width, c.height);
    }
  };

  useEffect(() => {
    playback.setFrameSink((bmp) => {
      lastBitmap.current?.close();
      lastBitmap.current = bmp;
      draw(bmp);
    });
    return () => playback.setFrameSink(null);
  }, []);

  useEffect(() => {
    const c = canvasRef.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.round(dw * dpr);
    c.height = Math.round(dh * dpr);
    draw(lastBitmap.current);
  }, [dw, dh]);

  // Paused preview: render the exact composited frame through the export render plan.
  // Latest-request-wins so scrubbing never queues stale renders.
  const inflight = useRef(false);
  const wanted = useRef<string>("");
  const revision = ps?.history.revision ?? 0;
  useEffect(() => {
    if (playing || !engineOk) return;
    const key = `${playhead}:${revision}:${previewScale}:${end}`;
    wanted.current = key;
    if (end === 0) {
      lastBitmap.current?.close();
      lastBitmap.current = null;
      draw(null);
      setRenderError(null);
      return;
    }
    const pump = async () => {
      if (inflight.current) return;
      inflight.current = true;
      try {
        while (true) {
          const k = wanted.current;
          const [f, , sc] = k.split(":").map(Number);
          try {
            const buf = await ipc.renderFrame(f, sc);
            const bmp = await createImageBitmap(new Blob([buf], { type: "image/jpeg" }));
            if (useStore.getState().playing) {
              bmp.close();
              break;
            }
            lastBitmap.current?.close();
            lastBitmap.current = bmp;
            draw(bmp);
            setRenderError(null);
          } catch (e) {
            setRenderError(errorMessage(e));
          }
          if (wanted.current === k) break;
        }
      } finally {
        inflight.current = false;
      }
    };
    void pump();
  }, [playhead, revision, previewScale, playing, end, engineOk]);

  const offlineUnderPlayhead =
    ps?.project.sequence.tracks
      .flatMap((t) => t.clips.filter((c) => c.start <= playhead && playhead < c.start + c.duration))
      .filter((c) => ps.offline.includes(c.assetId))
      .map((c) => ps.project.assets.find((a) => a.id === c.assetId)?.name)
      .filter(Boolean) ?? [];

  return (
    <section className="flex h-full min-h-0 min-w-0 flex-col bg-pc-panel" aria-label="Program viewer">
      <PanelHeader title={`Program${seq ? ` — ${seq.name}` : ""}`}>
        <span className="tabular mr-2 text-[11px] text-pc-faint">
          {seq ? `${seq.width}×${seq.height} · ${(fps.num / fps.den).toFixed(fps.den === 1 ? 0 : 3)} fps` : ""}
        </span>
      </PanelHeader>
      <div ref={areaRef} className="relative flex min-h-0 flex-1 items-center justify-center overflow-auto bg-[#08090a]">
        <div className="relative shrink-0" style={{ width: dw, height: dh }}>
          <canvas ref={canvasRef} style={{ width: dw, height: dh }} className="block bg-black" aria-label="Video preview" />
          {safeAreas && (
            <div className="pointer-events-none absolute inset-0">
              <div className="absolute border border-white/35" style={{ inset: "5% 5%" }} title="Action safe 90%" />
              <div className="absolute border border-dashed border-white/30" style={{ inset: "10% 10%" }} title="Title safe 80%" />
              <div className="absolute top-1/2 left-1/2 h-4 w-px -translate-x-1/2 -translate-y-1/2 bg-white/40" />
              <div className="absolute top-1/2 left-1/2 h-px w-4 -translate-x-1/2 -translate-y-1/2 bg-white/40" />
            </div>
          )}
          {end === 0 && (
            <div className="absolute inset-0 flex items-center justify-center text-[12px] text-pc-faint">
              {engineOk ? "Timeline is empty — drag media onto a track" : "FFmpeg not found — preview unavailable"}
            </div>
          )}
          {offlineUnderPlayhead.length > 0 && (
            <div className="absolute top-2 left-2 rounded bg-pc-danger/85 px-2 py-0.5 text-[11px] text-white">Media offline: {offlineUnderPlayhead.join(", ")}</div>
          )}
          {renderError && !playing && (
            <div className="absolute right-2 bottom-2 left-2 rounded bg-pc-danger/85 px-2 py-1 text-[11px] text-white" role="alert">
              Preview failed: {renderError.split("\n")[0]}
            </div>
          )}
        </div>
      </div>
      <div className="flex h-10 shrink-0 items-center gap-2 border-t border-pc-border px-2">
        <div className="tabular w-[104px] text-[13px] text-pc-accent-hi" aria-label="Current timecode" aria-live="off">
          {formatTimecode(playhead, fps)}
        </div>
        <div className="flex flex-1 items-center justify-center gap-0.5">
          <IconButton label="Go to start (Home)" onClick={() => ACTIONS["play.start"].run()}>
            <IconToStart />
          </IconButton>
          <IconButton label="Previous frame (←)" onClick={() => ACTIONS["play.prev"].run()}>
            <IconStepBack />
          </IconButton>
          <IconButton
            label={playing ? "Pause (Space)" : "Play (Space)"}
            disabled={!(ACTIONS["play.toggle"].enabled?.() ?? true)}
            onClick={() => ACTIONS["play.toggle"].run()}
            className="mx-1 h-8 w-9 bg-pc-raised text-pc-text"
          >
            {playing ? <IconPause size={18} /> : <IconPlay size={18} />}
          </IconButton>
          <IconButton label="Next frame (→)" onClick={() => ACTIONS["play.next"].run()}>
            <IconStepFwd />
          </IconButton>
          <IconButton label="Go to end (End)" onClick={() => ACTIONS["play.end"].run()}>
            <IconToEnd />
          </IconButton>
        </div>
        {playing && (
          <span className="tabular text-[10.5px] text-pc-faint" title="Frames skipped because they were not ready in time">
            dropped {dropped}
          </span>
        )}
        <IconButton label="Toggle safe areas (')" active={safeAreas} onClick={toggleSafe}>
          <span className="text-[10px] font-semibold">SAFE</span>
        </IconButton>
        <select
          aria-label="Viewer zoom"
          value={String(zoom)}
          onChange={(e) => setZoom(e.target.value === "fit" ? "fit" : (Number(e.target.value) as Zoom))}
          className="h-6 rounded-[var(--radius-pc)] border border-pc-border bg-pc-bg px-1 text-[11.5px]"
        >
          <option value="fit">Fit</option>
          <option value="0.5">50%</option>
          <option value="1">100%</option>
        </select>
        <select
          aria-label="Preview quality"
          value={String(previewScale)}
          onChange={(e) => setPreviewScale(Number(e.target.value))}
          title="Preview render resolution. Full uses the exact export resolution."
          className="h-6 rounded-[var(--radius-pc)] border border-pc-border bg-pc-bg px-1 text-[11.5px]"
        >
          <option value="1">Full</option>
          <option value="0.5">1/2</option>
          <option value="0.25">1/4</option>
        </select>
        <div className="tabular w-[104px] text-right text-[12px] text-pc-muted" aria-label="Sequence duration">
          {formatTimecode(end, fps)}
        </div>
      </div>
    </section>
  );
}
