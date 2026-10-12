import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { AboutDialog, ExportDialog, NewProjectDialog, ShortcutsDialog, StatusDialog } from "./components/Dialogs";
import { Inspector } from "./components/Inspector";
import { MediaBrowser } from "./components/MediaBrowser";
import { Splitter } from "./components/Splitter";
import { Timeline } from "./components/Timeline";
import { TopBar } from "./components/TopBar";
import { Viewer } from "./components/Viewer";
import { findAction } from "./lib/actions";
import { MEDIA_FILTER, useStore } from "./lib/store";

let initStarted = false;
const LAYOUT_KEY = "pulsecut.layout.v1";
const DEFAULT_LAYOUT = { left: 280, right: 300, bottom: 320 };

function useLayout() {
  const [layout, setLayout] = useState<typeof DEFAULT_LAYOUT>(() => {
    try {
      return { ...DEFAULT_LAYOUT, ...JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? "{}") };
    } catch {
      return DEFAULT_LAYOUT;
    }
  });
  useEffect(() => {
    const reset = () => setLayout(DEFAULT_LAYOUT);
    window.addEventListener("pulsecut:reset-layout", reset);
    return () => window.removeEventListener("pulsecut:reset-layout", reset);
  }, []);
  const persist = () => setLayout((l) => (localStorage.setItem(LAYOUT_KEY, JSON.stringify(l)), l));
  return { layout, setLayout, persist };
}

function StatusBar() {
  const engine = useStore((s) => s.engine);
  const ps = useStore((s) => s.ps);
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);
  return (
    <>
      <footer className="flex h-6 shrink-0 items-center gap-3 border-t border-pc-border bg-pc-panel px-2 text-[11px] text-pc-faint">
        <span className={engine?.ok ? "" : "text-pc-danger"} title={engine?.ffmpegPath ?? engine?.error ?? ""}>
          {engine?.ok ? `Engine: ${engine.version?.replace(/Copyright.*/, "").trim()}` : "Engine: FFmpeg not found"}
        </span>
        <span className="min-w-0 flex-1 truncate">{ps?.path ?? "Unsaved project"}</span>
        <span>{ps?.history.dirty ? "Unsaved changes" : "Saved"}</span>
      </footer>
      <div className="pointer-events-none fixed right-3 bottom-8 z-[60] flex w-[380px] flex-col gap-1.5" aria-live="polite">
        {toasts.map((t) => (
          <div
            key={t.id}
            role={t.kind === "error" ? "alert" : "status"}
            className={`pointer-events-auto flex items-start gap-2 rounded-[4px] border px-3 py-2 text-[12px] shadow-lg ${
              t.kind === "error" ? "border-pc-danger/60 bg-[#2a1517]" : t.kind === "ok" ? "border-pc-ok/50 bg-[#12251d]" : "border-pc-border-strong bg-pc-panel-2"
            }`}
          >
            <span className="flex-1 break-words select-text">{t.text}</span>
            <button type="button" aria-label="Dismiss" className="text-pc-faint hover:text-pc-text" onClick={() => dismiss(t.id)}>
              ✕
            </button>
          </div>
        ))}
      </div>
    </>
  );
}

export default function App() {
  const { layout, setLayout, persist } = useLayout();
  const dialog = useStore((s) => s.dialog);
  const reducedMotion = useStore((s) => s.reducedMotion);
  const engine = useStore((s) => s.engine);
  const [dropHover, setDropHover] = useState(false);

  useEffect(() => {
    if (initStarted) return;
    initStarted = true;
    void useStore.getState().init();
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("reduce-motion", reducedMotion);
  }, [reducedMotion]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (useStore.getState().dialog) return;
      const t = e.target as HTMLElement;
      if (t.closest("input, select, textarea, [role=menu]")) return;
      const a = findAction(e);
      if (!a) return;
      e.preventDefault();
      if (a.enabled?.() ?? true) void a.run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const win = getCurrentWindow();
    const unClose = win.onCloseRequested(async (e) => {
      if (!useStore.getState().ps?.history.dirty) return;
      e.preventDefault();
      if (await useStore.getState().confirmDiscard()) await win.destroy();
    });
    const unDrop = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type === "over" || ev.payload.type === "enter") setDropHover(true);
      else if (ev.payload.type === "leave") setDropHover(false);
      else if (ev.payload.type === "drop") {
        setDropHover(false);
        const ext = new Set(MEDIA_FILTER.extensions);
        const files = ev.payload.paths.filter((p) => ext.has(p.split(".").pop()?.toLowerCase() ?? ""));
        const projects = ev.payload.paths.filter((p) => p.toLowerCase().endsWith(".pulsecut"));
        if (files.length) void useStore.getState().importFiles(files);
        else if (projects.length) useStore.getState().toast("info", "Use File › Open Project to open .pulsecut files.");
        else if (ev.payload.paths.length) useStore.getState().toast("error", "None of the dropped files are supported media types.");
      }
    });
    return () => {
      void unClose.then((f) => f());
      void unDrop.then((f) => f());
    };
  }, []);

  useEffect(() => {
    void getCurrentWindow().setTitle("PulseCut");
  }, []);

  const clampSide = (v: number) => Math.min(560, Math.max(200, v));

  return (
    <div className="flex h-full flex-col">
      <TopBar />
      {engine && !engine.ok && (
        <div className="shrink-0 border-b border-pc-danger/40 bg-[#2a1517] px-3 py-1.5 text-[12px] text-pc-danger" role="alert">
          {engine.error}
        </div>
      )}
      <main className="flex min-h-0 flex-1 flex-col">
        <div className="flex min-h-0 flex-1">
          <div className="min-h-0 shrink-0" style={{ width: layout.left }}>
            <MediaBrowser />
          </div>
          <Splitter direction="vertical" label="Resize media panel" onDelta={(d) => setLayout((l) => ({ ...l, left: clampSide(l.left + d) }))} onDone={persist} />
          <div className="min-h-0 min-w-[360px] flex-1">
            <Viewer />
          </div>
          <Splitter direction="vertical" label="Resize inspector" onDelta={(d) => setLayout((l) => ({ ...l, right: clampSide(l.right - d) }))} onDone={persist} />
          <div className="min-h-0 shrink-0" style={{ width: layout.right }}>
            <Inspector />
          </div>
        </div>
        <Splitter
          direction="horizontal"
          label="Resize timeline"
          onDelta={(d) => setLayout((l) => ({ ...l, bottom: Math.min(window.innerHeight - 260, Math.max(170, l.bottom - d)) }))}
          onDone={persist}
        />
        <div className="min-h-0 shrink-0" style={{ height: layout.bottom }}>
          <Timeline />
        </div>
      </main>
      <StatusBar />
      {dialog === "new" && <NewProjectDialog />}
      {dialog === "export" && <ExportDialog />}
      {dialog === "shortcuts" && <ShortcutsDialog />}
      {dialog === "status" && <StatusDialog />}
      {dialog === "about" && <AboutDialog />}
      {dropHover && (
        <div className="pointer-events-none fixed inset-0 z-[70] flex items-center justify-center border-2 border-dashed border-pc-accent bg-pc-accent/10 text-[15px] text-pc-accent-hi">
          Drop media files to import
        </div>
      )}
    </div>
  );
}
