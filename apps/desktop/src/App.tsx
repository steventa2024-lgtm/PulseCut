import { useEffect, useState } from "react";
import { desktopShell } from "./api";
import { CommandStrip } from "./components/CommandStrip";
import { Inspector } from "./components/Inspector";
import { MediaBrowser } from "./components/MediaBrowser";
import { ProgramMonitor } from "./components/ProgramMonitor";
import { Timeline } from "./components/Timeline";
import { formatTimecode } from "./lib/time";
import { EditorProvider, useEditor } from "./state/EditorContext";
import { api } from "./api";

export function App() {
  if (!desktopShell()) {
    return (
      <main className="grid h-screen place-items-center bg-bg0 p-8 text-center text-sm text-muted">
        <div>
          <h1 className="mb-2 text-lg text-ink">PulseCut</h1>
          <p>This window is the desktop shell. Start it with <code>pnpm tauri dev</code> from <code>apps/desktop</code>.</p>
        </div>
      </main>
    );
  }
  return (
    <EditorProvider>
      <Shell />
    </EditorProvider>
  );
}

function Shell() {
  const editor = useEditor();
  const [layout, setLayout] = useState(() => readLayout());
  const [help, setHelp] = useState<string | null>(null);
  const offline = editor.state.project.assets.filter((asset) => asset.offline);

  useEffect(() => {
    document.documentElement.style.setProperty("--left", `${layout.left}px`);
    document.documentElement.style.setProperty("--right", `${layout.right}px`);
    document.documentElement.style.setProperty("--timeline", `${layout.timeline}px`);
    localStorage.setItem("pulsecut.layout.v1", JSON.stringify(layout));
  }, [layout]);

  useEffect(() => {
    if (editor.resetLayoutToken > 0) setLayout({ left: 280, right: 300, timeline: 280 });
  }, [editor.resetLayoutToken]);

  useEffect(() => {
    const onHelp = (event: Event) => setHelp((event as CustomEvent<string>).detail);
    document.addEventListener("pulsecut-help", onHelp);
    return () => document.removeEventListener("pulsecut-help", onHelp);
  }, []);

  const title = editor.state.path ?? editor.state.project.name;
  return (
    <div className="app">
      <CommandStrip />
      <div className="flex min-h-0 flex-col">
        {editor.recovery?.exists && (
          <div className="banner">
            <span>
              Recovery file found
              {editor.recovery.projectName ? ` for “${editor.recovery.projectName}”` : ""}. It was written automatically after an edit.
            </span>
            <button className="btn" onClick={() => void editor.restoreRecovery()}>
              Restore
            </button>
            <button className="btn" onClick={() => void editor.discardRecovery()}>
              Discard
            </button>
          </div>
        )}
        {offline.length > 0 && (
          <div className="banner error">
            Offline media: {offline.map((asset) => asset.name).join(", ")}. Relink it in the inspector before preview or export.
          </div>
        )}
        {editor.error && (
          <div className="banner error">
            <span className="min-w-0 flex-1">{editor.error}</span>
            <button className="btn" onClick={editor.clearError}>
              Dismiss
            </button>
          </div>
        )}
        <div className="workspace min-h-0 flex-1">
        <div className="upper">
          <MediaBrowser />
          <Splitter
            orientation="vertical"
            label="Resize media browser"
            onDrag={(delta) => setLayout((current) => ({ ...current, left: clamp(current.left + delta, 200, 460) }))}
          />
          <ProgramMonitor />
          <Splitter
            orientation="vertical"
            label="Resize inspector"
            onDrag={(delta) => setLayout((current) => ({ ...current, right: clamp(current.right - delta, 220, 480) }))}
          />
          <Inspector />
        </div>
        <Splitter
          orientation="horizontal"
          label="Resize timeline"
          onDrag={(delta) => setLayout((current) => ({ ...current, timeline: clamp(current.timeline - delta, 160, 520) }))}
        />
        <Timeline />
        </div>
      </div>
      <footer className="status">
        <span>{editor.state.dirty ? "Unsaved" : "Saved"}</span>
        <span className="truncate">{title}</span>
        <span>
          {editor.state.project.sequence.width}×{editor.state.project.sequence.height}
        </span>
        <span>{formatTimecode(editor.playheadUs, editor.state.project.sequence.fpsNum, editor.state.project.sequence.fpsDen)}</span>
        <span className="ml-auto truncate">{editor.engine?.ok ? editor.engine.version : editor.engine?.message ?? "Checking FFmpeg…"}</span>
      </footer>
      {editor.exportOpen && <ExportDialog />}
      {help && <HelpDialog kind={help} onClose={() => setHelp(null)} />}
    </div>
  );
}

function Splitter({
  orientation,
  label,
  onDrag,
}: {
  orientation: "vertical" | "horizontal";
  label: string;
  onDrag: (delta: number) => void;
}) {
  return (
    <div
      className={`splitter ${orientation}`}
      role="separator"
      aria-orientation={orientation === "vertical" ? "vertical" : "horizontal"}
      aria-label={label}
      tabIndex={0}
      onPointerDown={(event) => {
        event.preventDefault();
        let last = orientation === "vertical" ? event.clientX : event.clientY;
        const move = (ev: PointerEvent) => {
          const current = orientation === "vertical" ? ev.clientX : ev.clientY;
          onDrag(current - last);
          last = current;
        };
        const up = () => {
          window.removeEventListener("pointermove", move);
          window.removeEventListener("pointerup", up);
        };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      }}
    />
  );
}

function ExportDialog() {
  const editor = useEditor();
  const sequence = editor.state.project.sequence;
  const [path, setPath] = useState("");
  const [width, setWidth] = useState(sequence.width);
  const [height, setHeight] = useState(sequence.height);
  const [crf, setCrf] = useState(18);
  const [preset, setPreset] = useState("veryfast");
  const report = editor.exportReport;
  return (
    <div className="modal-back" role="presentation" onMouseDown={() => editor.setExportOpen(false)}>
      <form
        className="modal"
        role="dialog"
        aria-labelledby="export-title"
        onMouseDown={(event) => event.stopPropagation()}
        onSubmit={(event) => {
          event.preventDefault();
          if (!path) return;
          void editor.startExport({ output: path, width, height, crf, preset });
        }}
      >
        <h2 id="export-title" className="mb-3 text-base">
          Export H.264 MP4
        </h2>
        <p className="mb-3 text-xs text-muted">
          Audio is AAC at 160 kbps. The picture is scaled to fit this size with the sequence frame rate of {sequence.fpsNum}/{sequence.fpsDen}. Type a full path or use Browse.
        </p>
        <label className="prop">
          File
          <span className="flex gap-1">
            <input
              className="field"
              value={path}
              placeholder="/path/to/export.mp4"
              aria-label="Export file"
              onChange={(event) => setPath(event.target.value)}
            />
            <button
              type="button"
              className="btn"
              onClick={() => {
                void api.pickExport(`${sequence.name || "export"}.mp4`).then((picked) => {
                  if (picked) setPath(picked);
                });
              }}
            >
              Browse
            </button>
          </span>
        </label>
        <label className="prop">
          Width
          <input className="field" type="number" value={width} min={160} max={7680} step={2} onChange={(event) => setWidth(Number(event.target.value))} />
        </label>
        <label className="prop">
          Height
          <input className="field" type="number" value={height} min={160} max={4320} step={2} onChange={(event) => setHeight(Number(event.target.value))} />
        </label>
        <label className="prop">
          CRF
          <input className="field" type="number" value={crf} min={0} max={51} onChange={(event) => setCrf(Number(event.target.value))} />
        </label>
        <label className="prop">
          Preset
          <select className="field" value={preset} onChange={(event) => setPreset(event.target.value)}>
            <option value="ultrafast">ultrafast</option>
            <option value="veryfast">veryfast</option>
            <option value="medium">medium</option>
          </select>
        </label>
        {editor.render.active && editor.render.mode === "export" && (
          <div className="my-3">
            <div className="h-1 bg-black">
              <div className="h-1 bg-accent" style={{ width: `${Math.round(editor.render.ratio * 100)}%` }} />
            </div>
            <p className="mt-1 text-xs text-muted">{editor.render.message || "Exporting"}</p>
            <button type="button" className="btn mt-2" onClick={() => void editor.cancelRender()}>
              Cancel export
            </button>
          </div>
        )}
        {report && (
          <div className="my-3 text-xs leading-5 text-muted">
            <div>Wrote {report.output}</div>
            <div>
              {report.width}×{report.height} · {report.videoCodec} · {report.audioCodec ?? "no audio"} ·{" "}
              {(report.durationUs / 1_000_000).toFixed(3)} s · {report.bytes} bytes
            </div>
          </div>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" className="btn" onClick={() => editor.setExportOpen(false)}>
            Close
          </button>
          <button className="btn primary" type="submit" disabled={!path || editor.render.active}>
            Export
          </button>
        </div>
      </form>
    </div>
  );
}

function HelpDialog({ kind, onClose }: { kind: string; onClose: () => void }) {
  const editor = useEditor();
  return (
    <div className="modal-back" onMouseDown={onClose}>
      <div className="modal" role="dialog" onMouseDown={(event) => event.stopPropagation()}>
        {kind === "keys" && (
          <>
            <h2 className="mb-2 text-base">Keyboard shortcuts</h2>
            <ul className="space-y-1 text-sm text-muted">
              <li>Space — play or pause</li>
              <li>Left / Right — step one frame. Shift adds one second.</li>
              <li>Home / End — go to the start or end</li>
              <li>S — split the selected clip at the playhead</li>
              <li>Delete — delete the selected clip and its linked group</li>
              <li>N — toggle timeline snapping</li>
              <li>Ctrl+Z / Ctrl+Shift+Z — undo and redo</li>
              <li>Ctrl+S — save. Ctrl+I — import. Ctrl+E — export.</li>
              <li>Ctrl+= / Ctrl+- — zoom the timeline</li>
            </ul>
          </>
        )}
        {kind === "status" && (
          <>
            <h2 className="mb-2 text-base">Feature status</h2>
            <p className="mb-2 text-sm text-muted">
              This build is the first editing slice. Working actions are the ones in the menus. Transitions, titles, effects, color grading, captions, multicam, and interchange are not implemented.
            </p>
            <p className="text-sm text-muted">The ledger is in docs/FEATURE_MATRIX.md. Nothing in that list is implied by an empty button.</p>
          </>
        )}
        {kind === "about" && (
          <>
            <h2 className="mb-2 text-base">PulseCut 0.1.0</h2>
            <p className="text-sm text-muted">
              A free, local-first editor from ZeroPulse, licensed under GPL-3.0-or-later. Preview and export call the FFmpeg executable on this computer. FFmpeg is not bundled in this repository.
            </p>
            <p className="mt-2 text-xs text-faint">{editor.engine?.message}</p>
          </>
        )}
        <div className="mt-4 flex justify-end">
          <button className="btn" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

function readLayout() {
  const fallback = { left: 280, right: 300, timeline: 280 };
  try {
    const parsed = JSON.parse(localStorage.getItem("pulsecut.layout.v1") ?? "") as typeof fallback;
    if (!parsed || typeof parsed.left !== "number") return fallback;
    return parsed;
  } catch {
    return fallback;
  }
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
