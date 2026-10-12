import { useEffect, useState } from "react";
import { api } from "../api";
import { formatTimecode, parseTimecode } from "../lib/time";
import { useEditor } from "../state/EditorContext";
import type { Asset, Clip } from "../types";

export function Inspector() {
  const editor = useEditor();
  const sequence = editor.state.project.sequence;
  const located = locate(editor.state.project.sequence.tracks, editor.selectedClipId);
  return (
    <section className="panel" aria-label="Inspector">
      <div className="inspector-head">
        <strong className="text-ink">{located ? "Clip" : "Sequence"}</strong>
      </div>
      <div className="scroll flex-1 p-3 text-sm">
        {located ? (
          <ClipInspector clip={located.clip} asset={editor.state.project.assets.find((asset) => asset.id === located.clip.assetId)} audio={located.trackKind === "audio"} />
        ) : (
          <SequenceInspector />
        )}
        <p className="mt-4 text-[11px] leading-4 text-faint">
          Sequence {sequence.name} · {sequence.width}×{sequence.height} · {sequence.fpsNum}/{sequence.fpsDen}. Keyframes, effects, and color tools are not in this build.
        </p>
      </div>
    </section>
  );
}

function SequenceInspector() {
  const editor = useEditor();
  const sequence = editor.state.project.sequence;
  const [width, setWidth] = useState(String(sequence.width));
  const [height, setHeight] = useState(String(sequence.height));
  useEffect(() => {
    setWidth(String(sequence.width));
    setHeight(String(sequence.height));
  }, [sequence.width, sequence.height, sequence.id]);
  const commitSize = () => {
    const nextWidth = Number(width);
    const nextHeight = Number(height);
    if (!Number.isFinite(nextWidth) || !Number.isFinite(nextHeight)) return;
    void editor.dispatch({
      type: "setSequence",
      width: nextWidth,
      height: nextHeight,
      fpsNum: sequence.fpsNum,
      fpsDen: sequence.fpsDen,
    });
  };
  return (
    <div>
      <label className="prop">
        Width
        <input className="field" value={width} onChange={(event) => setWidth(event.target.value)} onBlur={commitSize} />
      </label>
      <label className="prop">
        Height
        <input className="field" value={height} onChange={(event) => setHeight(event.target.value)} onBlur={commitSize} />
      </label>
      <div className="mt-2 flex flex-wrap gap-1">
        {[
          [24, 1],
          [25, 1],
          [30, 1],
          [60, 1],
        ].map(([num, den]) => (
          <button
            key={num}
            className={`btn ${sequence.fpsNum === num && sequence.fpsDen === den ? "active" : ""}`}
            onClick={() =>
              void editor.dispatch({
                type: "setSequence",
                width: sequence.width,
                height: sequence.height,
                fpsNum: num,
                fpsDen: den,
              })
            }
          >
            {num} fps
          </button>
        ))}
      </div>
    </div>
  );
}

function ClipInspector({ clip, asset, audio }: { clip: Clip; asset: Asset | undefined; audio: boolean }) {
  const editor = useEditor();
  const { fpsNum, fpsDen } = editor.state.project.sequence;
  const [start, setStart] = useState(formatTimecode(clip.timelineStartUs, fpsNum, fpsDen));
  const [source, setSource] = useState(formatTimecode(clip.sourceInUs, fpsNum, fpsDen));
  const [duration, setDuration] = useState(formatTimecode(clip.durationUs, fpsNum, fpsDen));
  const [name, setName] = useState(clip.name);
  const [gain, setGain] = useState(clip.gainDb);
  useEffect(() => {
    setStart(formatTimecode(clip.timelineStartUs, fpsNum, fpsDen));
    setSource(formatTimecode(clip.sourceInUs, fpsNum, fpsDen));
    setDuration(formatTimecode(clip.durationUs, fpsNum, fpsDen));
    setGain(clip.gainDb);
    setName(clip.name);
  }, [clip.id, clip.name, clip.timelineStartUs, clip.sourceInUs, clip.durationUs, clip.gainDb, fpsNum, fpsDen]);

  const commit = () => {
    const timelineStartUs = parseTimecode(start, fpsNum, fpsDen);
    const sourceInUs = parseTimecode(source, fpsNum, fpsDen);
    const durationUs = parseTimecode(duration, fpsNum, fpsDen);
    if (timelineStartUs === null || sourceInUs === null || durationUs === null) {
      editor.clearError();
      return;
    }
    void editor.dispatch({
      type: "setClipTiming",
      clipId: clip.id,
      timelineStartUs,
      sourceInUs,
      durationUs,
    });
  };

  return (
    <div>
      <label className="prop">
        Name
        <input
          className="field"
          value={name}
          onChange={(event) => setName(event.target.value)}
          onBlur={() => {
            if (name !== clip.name) void editor.dispatch({ type: "setClipName", clipId: clip.id, name });
          }}
        />
      </label>
      <label className="prop">
        Start
        <input className="field" value={start} onChange={(event) => setStart(event.target.value)} onBlur={commit} />
      </label>
      <label className="prop">
        Source in
        <input className="field" value={source} onChange={(event) => setSource(event.target.value)} onBlur={commit} />
      </label>
      <label className="prop">
        Duration
        <input className="field" value={duration} onChange={(event) => setDuration(event.target.value)} onBlur={commit} />
      </label>
      {audio && (
        <label className="prop">
          Gain dB
          <input
            type="range"
            min={-60}
            max={12}
            step={0.5}
            value={gain}
            aria-label="Gain in decibels"
            onChange={(event) => setGain(Number(event.target.value))}
            onPointerUp={() => void editor.dispatch({ type: "setClipGain", clipId: clip.id, gainDb: gain })}
            onKeyUp={() => void editor.dispatch({ type: "setClipGain", clipId: clip.id, gainDb: gain })}
          />
        </label>
      )}
      {audio && <p className="text-xs text-muted">{gain.toFixed(1)} dB</p>}
      {clip.linkId && <p className="mt-2 text-xs text-muted">Linked to other clips. Move, trim, split, and delete stay together.</p>}
      {asset && (
        <div className="mt-3 text-xs leading-5 text-muted">
          <div>{asset.offline ? "Offline" : asset.path}</div>
          <div>
            {asset.videoCodec ?? "no video"}
            {asset.audioCodec ? ` · ${asset.audioCodec}` : ""}
            {asset.audioSampleRate ? ` · ${asset.audioSampleRate} Hz` : ""}
          </div>
          {asset.vfrSuspected && <div>Variable frame rate. Preview and export conform this clip to the sequence frame rate.</div>}
          {asset.hdr && <div>HDR source. This build does not tone-map it.</div>}
          <button
            className="btn mt-2"
            onClick={() => {
              void (async () => {
                const picked = await api.pickRelink();
                if (!picked || Array.isArray(picked)) return;
                const probed = await api.probeMedia(picked);
                await editor.dispatch({ type: "updateAsset", asset: { ...probed, id: asset.id } });
              })();
            }}
          >
            Relink file
          </button>
        </div>
      )}
    </div>
  );
}

function locate(tracks: { kind: "video" | "audio"; clips: Clip[] }[], id: string | null) {
  if (!id) return null;
  for (const track of tracks) {
    const clip = track.clips.find((item) => item.id === id);
    if (clip) return { clip, trackKind: track.kind };
  }
  return null;
}
