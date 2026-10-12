import { useEffect, useState } from "react";
import { api } from "../api";
import { formatTimecode } from "../lib/time";
import { useEditor } from "../state/EditorContext";

export function ProgramMonitor() {
  const editor = useEditor();
  const { fpsNum, fpsDen, width, height } = editor.state.project.sequence;
  const [safe, setSafe] = useState(false);
  const previewFresh = editor.preview?.revision === editor.state.revision;
  const src = editor.preview && previewFresh ? api.assetUrl(editor.preview.path) : "";

  useEffect(() => {
    const video = editor.videoRef.current;
    if (!video) return;
    if (video.src !== src) video.src = src;
  }, [editor.videoRef, src]);

  return (
    <section className="panel" aria-label="Program monitor">
      <div className="monitor-bar">
        <span>Program</span>
        <span className="text-faint">
          {width}×{height} · {fpsNum}/{fpsDen}
          {editor.preview ? ` · proxy` : ""}
        </span>
        <button className={`icon-btn ${safe ? "active" : ""}`} aria-pressed={safe} onClick={() => setSafe((value) => !value)}>
          Safe
        </button>
        <div className="ml-auto flex items-center gap-1">
          <button className="icon-btn" aria-label="Step back one frame" onClick={() => editor.step(-1)}>
            Prev
          </button>
          <button className="btn primary" onClick={() => void editor.togglePlay()}>
            {editor.playing ? "Pause" : "Play"}
          </button>
          <button className="icon-btn" aria-label="Step forward one frame" onClick={() => editor.step(1)}>
            Next
          </button>
        </div>
      </div>
      <div className="monitor">
        <video
          ref={editor.videoRef}
          onLoadedData={editor.onVideoReady}
          onTimeUpdate={(event) => editor.notePlayhead(event.currentTarget.currentTime * 1_000_000)}
          onPlay={() => editor.notePlaying(true)}
          onPause={() => editor.notePlaying(false)}
          onEnded={() => editor.notePlaying(false)}
        />
        {safe && (
          <div className="safe" aria-hidden="true">
            <span />
          </div>
        )}
        <div className="hud">{formatTimecode(editor.playheadUs, fpsNum, fpsDen)}</div>
        {editor.render.active && editor.render.mode === "preview" && (
          <div className="progress-line" aria-label="Preview progress">
            <i style={{ width: `${Math.round(editor.render.ratio * 100)}%` }} />
          </div>
        )}
        {!src && (
          <p className="absolute px-6 text-center text-sm text-faint">
            {editor.render.active && editor.render.mode === "preview"
              ? `Rendering preview ${editor.render.message}`
              : "Press Play to render a proxy of the timeline and hear the mixed audio."}
          </p>
        )}
        {editor.preview && !previewFresh && !editor.render.active && (
          <p className="absolute top-2 rounded-sm bg-black/70 px-2 py-1 text-xs text-[#e2b340]">
            Preview is out of date. Press Play to render the current timeline.
          </p>
        )}
      </div>
    </section>
  );
}
