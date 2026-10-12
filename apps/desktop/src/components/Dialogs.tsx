import { Channel } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { ACTIONS } from "../lib/actions";
import { errorMessage, ipc } from "../lib/ipc";
import { sequenceEnd, useStore } from "../lib/store";
import { formatDuration, formatFps, framesToSeconds } from "../lib/time";
import type { ExportEvent, ExportQuality, VerifyCheck } from "../lib/types";
import logo from "../logo.svg";
import { Button, Modal } from "./ui";
import { withExtension } from "../lib/paths";

const PRESETS = [
  { label: "HD 1080p · 1920×1080", w: 1920, h: 1080 },
  { label: "HD 720p · 1280×720", w: 1280, h: 720 },
  { label: "UHD 4K · 3840×2160", w: 3840, h: 2160 },
  { label: "Vertical 9:16 · 1080×1920", w: 1080, h: 1920 },
  { label: "Square 1:1 · 1080×1080", w: 1080, h: 1080 },
  { label: "Small 16:9 · 640×360", w: 640, h: 360 },
];
const RATES = [
  { label: "23.976", num: 24000, den: 1001 },
  { label: "24", num: 24, den: 1 },
  { label: "25", num: 25, den: 1 },
  { label: "29.97", num: 30000, den: 1001 },
  { label: "30", num: 30, den: 1 },
  { label: "50", num: 50, den: 1 },
  { label: "60", num: 60, den: 1 },
];

const field = "h-7 w-full rounded-[var(--radius-pc)] border border-pc-border bg-pc-bg px-2 text-[12.5px] focus:border-pc-accent focus:outline-none";

export function NewProjectDialog() {
  const close = () => useStore.getState().setDialog(null);
  const [name, setName] = useState("Untitled");
  const [preset, setPreset] = useState(0);
  const [rate, setRate] = useState(4);
  const [err, setErr] = useState<string | null>(null);
  const create = async () => {
    try {
      const p = PRESETS[preset];
      const r = RATES[rate];
      await useStore.getState().stopPlayback();
      const ps = await ipc.newProject({ name, width: p.w, height: p.h, fpsNum: r.num, fpsDen: r.den });
      useStore.setState({ selection: [], selectedAsset: null, playhead: 0 });
      useStore.getState().setState(ps);
      close();
    } catch (e) {
      setErr(errorMessage(e));
    }
  };
  return (
    <Modal title="New Project" onClose={close} width={440}>
      <form
        className="flex flex-col gap-3 text-[12.5px]"
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <label className="flex flex-col gap-1">
          <span className="text-pc-muted">Project name</span>
          <input className={field} value={name} onChange={(e) => setName(e.target.value)} maxLength={120} />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-pc-muted">Frame size</span>
          <select className={field} value={preset} onChange={(e) => setPreset(Number(e.target.value))}>
            {PRESETS.map((p, i) => (
              <option key={p.label} value={i}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-pc-muted">Frame rate (fps)</span>
          <select className={field} value={rate} onChange={(e) => setRate(Number(e.target.value))}>
            {RATES.map((r, i) => (
              <option key={r.label} value={i}>
                {r.label}
              </option>
            ))}
          </select>
        </label>
        <p className="text-[11.5px] text-pc-faint">Audio: 48 kHz stereo. Clips with other frame rates are resampled to the sequence rate.</p>
        {err && <p className="text-pc-danger">{err}</p>}
        <div className="flex justify-end gap-2 pt-1">
          <Button onClick={close}>Cancel</Button>
          <Button variant="primary" type="submit">
            Create
          </Button>
        </div>
      </form>
    </Modal>
  );
}

type ExportPhase =
  | { kind: "setup" }
  | { kind: "running"; fraction: number; speed: string | null; out: number; total: number; startedAt: number }
  | { kind: "verifying" }
  | { kind: "done"; path: string; encoder: string; elapsed: number; pass: boolean; checks: VerifyCheck[] }
  | { kind: "failed"; message: string; log: string }
  | { kind: "cancelled" };

export function ExportDialog() {
  const ps = useStore((s) => s.ps)!;
  const engine = useStore((s) => s.engine);
  const [path, setPath] = useState<string>("");
  const [quality, setQuality] = useState<ExportQuality>("standard");
  const [scale, setScale] = useState(1);
  const [phase, setPhase] = useState<ExportPhase>({ kind: "setup" });
  const running = phase.kind === "running" || phase.kind === "verifying";
  const close = () => !running && useStore.getState().setDialog(null);
  const seq = ps.project.sequence;
  const end = sequenceEnd(ps);
  const seconds = framesToSeconds(end, seq.frameRate);
  const w = Math.round((seq.width * scale) / 2) * 2;
  const h = Math.round((seq.height * scale) / 2) * 2;
  const offlineNames = ps.offline.map((id) => ps.project.assets.find((a) => a.id === id)?.name).filter(Boolean);
  const channelRef = useRef<Channel<ExportEvent> | null>(null);

  useEffect(() => {
    if (ps.path && !path) setPath(ps.path.replace(/\.pulsecut$/i, ".mp4"));
  }, [ps.path, path]);

  const choose = async () => {
    const picked = await save({ title: "Export video", defaultPath: path || `${ps.project.name}.mp4`, filters: [{ name: "MPEG-4 video", extensions: ["mp4"] }, { name: "QuickTime", extensions: ["mov"] }] });
    if (picked) setPath(withExtension(picked, ["mp4", "mov"]));
  };

  const start = async () => {
    if (!path) return choose();
    const ch = new Channel<ExportEvent>();
    channelRef.current = ch;
    const startedAt = performance.now();
    setPhase({ kind: "running", fraction: 0, speed: null, out: 0, total: seconds, startedAt });
    ch.onmessage = (ev) => {
      if (ev.type === "progress") setPhase({ kind: "running", fraction: ev.fraction, speed: ev.speed, out: ev.outSeconds, total: ev.totalSeconds, startedAt });
      else if (ev.type === "verifying") setPhase({ kind: "verifying" });
      else if (ev.type === "done") {
        setPhase({ kind: "done", path: ev.path, encoder: ev.encoder, elapsed: ev.elapsedSeconds, pass: ev.report.pass, checks: ev.report.checks });
        useStore.getState().toast(ev.report.pass ? "ok" : "error", ev.report.pass ? "Export finished and verified" : "Export finished but verification failed");
      } else if (ev.type === "failed") setPhase({ kind: "failed", message: ev.message, log: ev.log });
      else if (ev.type === "cancelled") setPhase({ kind: "cancelled" });
    };
    try {
      await ipc.exportStart(path, { quality, scale, audioBitrateKbps: 192 }, ch);
    } catch (e) {
      setPhase({ kind: "failed", message: errorMessage(e), log: "" });
    }
  };

  return (
    <Modal title="Export Video" onClose={close} width={560}>
      <div className="flex flex-col gap-3 text-[12.5px]">
        <div className="grid grid-cols-[110px_1fr] items-center gap-x-3 gap-y-2.5">
          <span className="text-pc-muted">Destination</span>
          <div className="flex gap-2">
            <input className={`${field} tabular text-[11.5px]`} value={path} readOnly placeholder="Choose a file…" aria-label="Export file path" />
            <Button onClick={choose} disabled={running}>
              Choose…
            </Button>
          </div>
          <span className="text-pc-muted">Format</span>
          <span>{path.toLowerCase().endsWith(".mov") ? "QuickTime (.mov)" : "MPEG-4 (.mp4)"} · H.264 + AAC 192 kbps</span>
          <span className="text-pc-muted">Quality</span>
          <select className={field} value={quality} disabled={running} onChange={(e) => setQuality(e.target.value as ExportQuality)}>
            <option value="high">High (CRF 18, slow preset)</option>
            <option value="standard">Standard (CRF 21, medium preset)</option>
            <option value="draft">Draft (CRF 28, very fast)</option>
          </select>
          <span className="text-pc-muted">Resolution</span>
          <select className={field} value={scale} disabled={running} onChange={(e) => setScale(Number(e.target.value))}>
            <option value={1}>
              Sequence ({seq.width}×{seq.height})
            </option>
            <option value={0.5}>
              Half ({Math.round(seq.width / 4) * 2}×{Math.round(seq.height / 4) * 2})
            </option>
          </select>
          <span className="text-pc-muted">Summary</span>
          <span className="tabular text-pc-muted">
            {w}×{h} · {formatFps(seq.frameRate)} fps · {formatDuration(seconds)} · encoder {engine?.h264Encoder ?? "unavailable"}
          </span>
        </div>
        {offlineNames.length > 0 && <p className="rounded bg-pc-danger/15 p-2 text-pc-danger">Relink offline media before exporting: {offlineNames.join(", ")}</p>}

        {phase.kind === "running" && (
          <div className="flex flex-col gap-1.5" aria-live="polite">
            <div className="h-2 overflow-hidden rounded bg-pc-bg" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(phase.fraction * 100)}>
              <div className="h-full bg-pc-accent transition-[width]" style={{ width: `${phase.fraction * 100}%` }} />
            </div>
            <div className="tabular flex justify-between text-[11.5px] text-pc-muted">
              <span>
                {Math.round(phase.fraction * 100)}% · {phase.out.toFixed(1)} / {phase.total.toFixed(1)} s
              </span>
              <span>
                {phase.speed ? `speed ${phase.speed}` : "starting…"}
                {phase.fraction > 0.02 && ` · ~${Math.max(0, ((performance.now() - phase.startedAt) / 1000) * (1 / phase.fraction - 1)).toFixed(0)} s left`}
              </span>
            </div>
          </div>
        )}
        {phase.kind === "verifying" && <p className="text-pc-accent-hi">Verifying the exported file (probe + full decode)…</p>}
        {phase.kind === "cancelled" && <p className="text-pc-warn">Export cancelled. No file was written.</p>}
        {phase.kind === "failed" && (
          <div className="flex flex-col gap-1">
            <p className="text-pc-danger">Export failed: {phase.message}</p>
            {phase.log && <pre className="max-h-40 overflow-auto rounded bg-pc-bg p-2 text-[10.5px] whitespace-pre-wrap text-pc-muted select-text">{phase.log.slice(-4000)}</pre>}
          </div>
        )}
        {phase.kind === "done" && (
          <div className="flex flex-col gap-2">
            <p className={phase.pass ? "text-pc-ok" : "text-pc-danger"}>
              {phase.pass ? "Export complete and verified." : "Export complete, but some checks failed."} ({phase.elapsed.toFixed(1)} s with {phase.encoder})
            </p>
            <p className="tabular text-[11.5px] break-all text-pc-muted select-text">{phase.path}</p>
            <table className="tabular w-full text-left text-[11.5px]">
              <thead className="text-pc-faint">
                <tr>
                  <th className="py-0.5 font-normal">Check</th>
                  <th className="font-normal">Expected</th>
                  <th className="font-normal">Actual</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {phase.checks.map((c) => (
                  <tr key={c.name} className="border-t border-pc-border">
                    <td className="py-0.5">{c.name}</td>
                    <td className="text-pc-muted">{c.expected}</td>
                    <td>{c.actual}</td>
                    <td className={c.pass ? "text-pc-ok" : "text-pc-danger"}>{c.pass ? "PASS" : "FAIL"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <div className="flex justify-end gap-2 pt-1">
          {running ? (
            <Button variant="danger" disabled={phase.kind === "verifying"} onClick={() => void ipc.exportCancel()}>
              Cancel Export
            </Button>
          ) : (
            <>
              <Button onClick={close}>Close</Button>
              <Button variant="primary" disabled={end === 0 || offlineNames.length > 0 || !engine?.h264Encoder} onClick={start}>
                {phase.kind === "setup" ? "Export" : "Export Again"}
              </Button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
}

export function ShortcutsDialog() {
  const close = () => useStore.getState().setDialog(null);
  const rows = Object.values(ACTIONS).filter((a) => a.keys?.length);
  return (
    <Modal title="Keyboard Shortcuts" onClose={close} width={520}>
      <table className="w-full text-[12.5px]">
        <tbody>
          {rows.map((a) => (
            <tr key={a.id} className="border-b border-pc-border last:border-0">
              <td className="py-1">{a.label}</td>
              <td className="tabular py-1 text-right text-pc-muted">{a.keys!.join("  /  ")}</td>
            </tr>
          ))}
          <tr>
            <td className="py-1">Zoom timeline at cursor</td>
            <td className="tabular py-1 text-right text-pc-muted">Ctrl + mouse wheel</td>
          </tr>
          <tr>
            <td className="py-1">Add to / remove from selection</td>
            <td className="tabular py-1 text-right text-pc-muted">Shift/Ctrl + click</td>
          </tr>
        </tbody>
      </table>
    </Modal>
  );
}

const STATUS: [string, string][] = [
  ["Import MP4/MOV/MKV/WebM, audio, stills (FFmpeg-decodable)", "Implemented"],
  ["Media metadata, VFR/HDR/rotation notices, offline media + relink", "Implemented"],
  ["Multitrack timeline: add, move, trim, split, delete, ripple delete", "Implemented"],
  ["Linked audio/video, snapping, zoom, mute/solo/lock/hide", "Implemented"],
  ["Undo/redo (exact snapshots), save/open .pulsecut, autosave recovery", "Implemented"],
  ["Preview stills and playback rendered by the export graph", "Implemented"],
  ["Clip opacity, scale, position, gain", "Implemented"],
  ["H.264/AAC MP4/MOV export with progress, cancel, verification", "Implemented"],
  ["Transitions, titles, keyframes, fades, effects", "Planned (Phase 2)"],
  ["Proxies, cached playback, hardware decode/encode", "Planned (Phase 2–3)"],
  ["Color grading, scopes, masks, keying, tracking", "Planned (Phase 3)"],
  ["Captions, local AI transcription", "Planned (Phase 4)"],
];

export function StatusDialog() {
  const close = () => useStore.getState().setDialog(null);
  return (
    <Modal title="Feature Status" onClose={close} width={600}>
      <p className="mb-2 text-[12px] text-pc-muted">What this build actually does. See docs/FEATURE_MATRIX.md for the full ledger.</p>
      <table className="w-full text-[12.5px]">
        <tbody>
          {STATUS.map(([f, s]) => (
            <tr key={f} className="border-b border-pc-border last:border-0">
              <td className="py-1 pr-3">{f}</td>
              <td className={`py-1 text-right whitespace-nowrap ${s === "Implemented" ? "text-pc-ok" : "text-pc-faint"}`}>{s}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Modal>
  );
}

export function AboutDialog() {
  const close = () => useStore.getState().setDialog(null);
  const engine = useStore((s) => s.engine);
  return (
    <Modal title="About PulseCut" onClose={close} width={460}>
      <div className="flex flex-col items-center gap-2 text-center text-[12.5px]">
        <img src={logo} alt="" className="h-16 w-16" />
        <div className="text-[15px] font-semibold">PulseCut 0.1.0</div>
        <p className="text-pc-muted">A free, open-source, local-first video editor by ZeroPulse.</p>
        <p className="text-pc-muted">Licensed under the GNU GPL v3.0 or later. No accounts, no watermarks, no telemetry.</p>
        <p className="tabular text-[11px] break-all text-pc-faint">
          Media engine: {engine?.ok ? `${engine.version} (${engine.ffmpegPath})` : engine?.error ?? "unknown"}
        </p>
      </div>
    </Modal>
  );
}
