import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { api } from "../api";
import { formatTimecode, sequenceDurationUs, snapUs } from "../lib/time";
import { useEditor } from "../state/EditorContext";
import type { Clip, Track } from "../types";

const waves = new Map<string, number[]>();

export function Timeline() {
  const editor = useEditor();
  const sequence = editor.state.project.sequence;
  const durationUs = Math.max(sequenceDurationUs(sequence.tracks), 20_000_000);
  const width = (durationUs / 1_000_000) * editor.pps;
  const scrollRef = useRef<HTMLDivElement>(null);
  const [snapLine, setSnapLine] = useState<number | null>(null);
  const [ghost, setGhost] = useState<{ id: string; start: number } | null>(null);
  const video = sequence.tracks.filter((track) => track.kind === "video").slice().reverse();
  const audio = sequence.tracks.filter((track) => track.kind === "audio");
  const rows = [...video, ...audio];

  useEffect(() => {
    if (!editor.playing || !scrollRef.current) return;
    const scroller = scrollRef.current;
    const x = 132 + (editor.playheadUs / 1_000_000) * editor.pps;
    if (x > scroller.scrollLeft + scroller.clientWidth - 40) {
      scroller.scrollLeft = x - 80;
    }
  }, [editor.playheadUs, editor.playing, editor.pps]);

  const pointsFor = (clip: Clip) => {
    const points = [0, editor.playheadUs];
    for (const track of sequence.tracks) {
      for (const other of track.clips) {
        if (other.id === clip.id || other.linkId === clip.linkId) continue;
        points.push(other.timelineStartUs, other.timelineStartUs + other.durationUs);
      }
    }
    return points;
  };

  return (
    <section className="timeline" aria-label="Timeline">
      <div className="timeline-bar">
        <span>Timeline</span>
        <button className="icon-btn" aria-pressed={editor.snap} onClick={() => editor.setSnap(!editor.snap)}>
          Snap {editor.snap ? "on" : "off"}
        </button>
        <button className="icon-btn" onClick={() => editor.setPps((value) => Math.max(20, value - 20))} aria-label="Zoom timeline out">
          −
        </button>
        <input
          type="range"
          min={20}
          max={400}
          value={editor.pps}
          aria-label="Timeline zoom"
          onChange={(event) => editor.setPps(Number(event.target.value))}
        />
        <button className="icon-btn" onClick={() => editor.setPps((value) => Math.min(400, value + 20))} aria-label="Zoom timeline in">
          +
        </button>
        <button className="btn" onClick={() => void editor.dispatch({ type: "addTrack", kind: "video" })}>
          Add video
        </button>
        <button className="btn" onClick={() => void editor.dispatch({ type: "addTrack", kind: "audio" })}>
          Add audio
        </button>
        <span className="ml-auto text-faint">{formatTimecode(editor.playheadUs, sequence.fpsNum, sequence.fpsDen)}</span>
      </div>
      <div className="timeline-scroll" ref={scrollRef}>
        <div style={{ width: width + 132 }}>
          <div className="track-row">
            <div className="track-head text-xs text-faint">Time</div>
            <div
              className="ruler"
              style={{ width }}
              onPointerDown={(event) => {
                const rect = event.currentTarget.getBoundingClientRect();
                editor.seek(Math.max(0, ((event.clientX - rect.left) / editor.pps) * 1_000_000));
              }}
            >
              {ticks(durationUs, editor.pps).map((tick) => (
                <span key={tick} className="absolute top-1 text-[10px] text-faint" style={{ left: (tick / 1_000_000) * editor.pps }}>
                  {formatTimecode(tick, sequence.fpsNum, sequence.fpsDen).slice(0, 8)}
                </span>
              ))}
              <Playhead us={editor.playheadUs} pps={editor.pps} />
            </div>
          </div>
          {rows.map((track) => (
            <TrackRow
              key={track.id}
              track={track}
              width={width}
              ghost={ghost}
              snapLine={snapLine}
              onDropClip={(assetId, timeUs) => {
                const asset = editor.state.project.assets.find((item) => item.id === assetId);
                if (asset) void editor.placeAsset(asset, track, timeUs);
              }}
              onMove={(clip, start) => {
                const snapped = editor.snap
                  ? snapUs(start, pointsFor(clip), (8 / editor.pps) * 1_000_000, sequence.fpsNum, sequence.fpsDen)
                  : snapUs(start, [], -1, sequence.fpsNum, sequence.fpsDen);
                setGhost({ id: clip.id, start: snapped.timeUs });
                setSnapLine(snapped.snappedTo);
                return snapped.timeUs;
              }}
              onMoveEnd={() => {
                setGhost(null);
                setSnapLine(null);
              }}
            />
          ))}
        </div>
      </div>
    </section>
  );
}

function TrackRow({
  track,
  width,
  ghost,
  snapLine,
  onDropClip,
  onMove,
  onMoveEnd,
}: {
  track: Track;
  width: number;
  ghost: { id: string; start: number } | null;
  snapLine: number | null;
  onDropClip: (assetId: string, timeUs: number) => void;
  onMove: (clip: Clip, start: number) => number;
  onMoveEnd: () => void;
}) {
  const editor = useEditor();
  const laneRef = useRef<HTMLDivElement>(null);
  return (
    <div className="track-row">
      <div className="track-head">
        <span className="text-xs">{track.name}</span>
        <span className="flex gap-1">
          <button
            className="icon-btn"
            aria-pressed={track.muted}
            aria-label={track.muted ? `Unmute ${track.name}` : `Mute ${track.name}`}
            onClick={() => void editor.dispatch({ type: "setTrackMuted", trackId: track.id, muted: !track.muted })}
          >
            {track.muted ? "Muted" : "Mute"}
          </button>
          <button
            className="icon-btn"
            aria-pressed={track.locked}
            aria-label={track.locked ? `Unlock ${track.name}` : `Lock ${track.name}`}
            onClick={() => void editor.dispatch({ type: "setTrackLocked", trackId: track.id, locked: !track.locked })}
          >
            {track.locked ? "Locked" : "Lock"}
          </button>
        </span>
      </div>
      <div
        ref={laneRef}
        className={`lane ${track.kind}`}
        style={{ width }}
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          const assetId = event.dataTransfer.getData("application/x-pulsecut-asset");
          if (!assetId || !laneRef.current) return;
          const rect = laneRef.current.getBoundingClientRect();
          const timeUs = ((event.clientX - rect.left) / editor.pps) * 1_000_000;
          onDropClip(assetId, Math.max(0, timeUs));
        }}
        onPointerDown={(event) => {
          if (event.target !== event.currentTarget) return;
          const rect = event.currentTarget.getBoundingClientRect();
          editor.seek(Math.max(0, ((event.clientX - rect.left) / editor.pps) * 1_000_000));
          editor.selectClip(null);
        }}
      >
        {track.clips.map((clip) => (
          <ClipView
            key={clip.id}
            clip={clip}
            track={track}
            start={ghost?.id === clip.id ? ghost.start : clip.timelineStartUs}
            selected={editor.selectedClipId === clip.id}
            onMove={onMove}
            onMoveEnd={onMoveEnd}
          />
        ))}
        {snapLine !== null && <div className="playhead" style={{ left: (snapLine / 1_000_000) * editor.pps }} />}
        <Playhead us={editor.playheadUs} pps={editor.pps} />
      </div>
    </div>
  );
}

function ClipView({
  clip,
  track,
  start,
  selected,
  onMove,
  onMoveEnd,
}: {
  clip: Clip;
  track: Track;
  start: number;
  selected: boolean;
  onMove: (clip: Clip, start: number) => number;
  onMoveEnd: () => void;
}) {
  const editor = useEditor();
  const asset = editor.state.project.assets.find((item) => item.id === clip.assetId);
  const left = (start / 1_000_000) * editor.pps;
  const width = Math.max(8, (clip.durationUs / 1_000_000) * editor.pps);
  return (
    <div
      className={`clip ${track.kind} ${selected ? "selected" : ""}`}
      style={{ left, width }}
      onPointerDown={(event) => {
        if (track.locked || event.button !== 0) return;
        const target = event.target as HTMLElement;
        if (target.dataset.handle) return;
        event.stopPropagation();
        editor.selectClip(clip.id);
        const originX = event.clientX;
        const origin = clip.timelineStartUs;
        const move = (ev: PointerEvent) => {
          const delta = ((ev.clientX - originX) / editor.pps) * 1_000_000;
          onMove(clip, origin + delta);
        };
        const up = (ev: PointerEvent) => {
          window.removeEventListener("pointermove", move);
          window.removeEventListener("pointerup", up);
          const delta = ((ev.clientX - originX) / editor.pps) * 1_000_000;
          const next = onMove(clip, origin + delta);
          onMoveEnd();
          if (Math.round(next) !== clip.timelineStartUs) {
            void editor.dispatch({ type: "moveClip", clipId: clip.id, timelineStartUs: Math.round(next) });
          }
        };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      }}
    >
      <span
        className="handle left"
        data-handle="start"
        onPointerDown={(event) => beginTrim(event, "start", clip, track, editor, onMoveEnd)}
      />
      <span className="truncate">{clip.name}</span>
      {track.kind === "audio" && asset && !asset.offline && <Waveform assetPath={asset.path} sourceIn={clip.sourceInUs} duration={clip.durationUs} />}
      <span
        className="handle right"
        data-handle="end"
        onPointerDown={(event) => beginTrim(event, "end", clip, track, editor, onMoveEnd)}
      />
    </div>
  );
}

function beginTrim(
  event: ReactPointerEvent<HTMLSpanElement>,
  edge: "start" | "end",
  clip: Clip,
  track: Track,
  editor: ReturnType<typeof useEditor>,
  onMoveEnd: () => void,
) {
  if (track.locked) return;
  event.stopPropagation();
  event.preventDefault();
  editor.selectClip(clip.id);
  const originX = event.clientX;
  const up = (ev: PointerEvent) => {
    window.removeEventListener("pointerup", up);
    onMoveEnd();
    const delta = ((ev.clientX - originX) / editor.pps) * 1_000_000;
    const time = edge === "start" ? clip.timelineStartUs + delta : clip.timelineStartUs + clip.durationUs + delta;
    void editor.dispatch({ type: "trimClip", clipId: clip.id, edge, timelineUs: Math.max(0, Math.round(time)) });
  };
  window.addEventListener("pointerup", up);
}

function Waveform({ assetPath, sourceIn, duration }: { assetPath: string; sourceIn: number; duration: number }) {
  const key = `${assetPath}:${sourceIn}:${duration}`;
  const [peaks, setPeaks] = useState<number[] | undefined>(waves.get(key));
  useEffect(() => {
    if (waves.has(key)) return;
    let cancelled = false;
    void api.waveform(assetPath, sourceIn, duration, 80).then(
      (data) => {
        waves.set(key, data);
        if (!cancelled) setPeaks(data);
      },
      () => {
        waves.set(key, []);
        if (!cancelled) setPeaks([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [assetPath, duration, key, sourceIn]);
  if (!peaks || peaks.length === 0) return null;
  const d = peaks
    .map((peak, index) => {
      const x = (index / (peaks.length - 1 || 1)) * 100;
      const y = 50 - peak * 45;
      return `${index === 0 ? "M" : "L"}${x},${y}`;
    })
    .join(" ");
  return (
    <svg className="wave" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
      <path d={d} fill="none" stroke="rgba(255,255,255,0.75)" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function Playhead({ us, pps }: { us: number; pps: number }) {
  return <div className="playhead" style={{ left: (us / 1_000_000) * pps }} />;
}

function ticks(durationUs: number, pps: number) {
  const step = pps > 140 ? 500_000 : 1_000_000;
  const marks: number[] = [];
  for (let time = 0; time <= durationUs; time += step) marks.push(time);
  return marks;
}
