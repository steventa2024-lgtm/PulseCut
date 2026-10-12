import { useEffect, useRef, useState } from "react";
import { useEditor } from "../state/EditorContext";

type Item = { id: string; label: string; shortcut?: string; disabled?: boolean; run: () => void };
type Menu = { id: string; label: string; items: Item[] };

export function CommandStrip() {
  const editor = useEditor();
  const [open, setOpen] = useState<string | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const clip = editor.state.project.sequence.tracks
    .flatMap((track) => track.clips)
    .find((item) => item.id === editor.selectedClipId);

  const menus: Menu[] = [
    {
      id: "file",
      label: "File",
      items: [
        { id: "new", label: "New project", shortcut: "", run: () => void editor.newProject() },
        { id: "open", label: "Open…", run: () => void editor.openProject() },
        { id: "save", label: "Save", shortcut: "Ctrl+S", run: () => void editor.saveProject(false) },
        { id: "save-as", label: "Save as…", run: () => void editor.saveProject(true) },
        { id: "import", label: "Import media…", shortcut: "Ctrl+I", run: () => void editor.importMedia() },
        { id: "sample", label: "Load sample timeline", run: () => void editor.loadSample() },
        { id: "export", label: "Export MP4…", shortcut: "Ctrl+E", run: () => editor.setExportOpen(true) },
        { id: "quit", label: "Quit", run: () => void editor.quit() },
      ],
    },
    {
      id: "edit",
      label: "Edit",
      items: [
        { id: "undo", label: "Undo", shortcut: "Ctrl+Z", disabled: !editor.state.canUndo, run: () => void editor.undo() },
        { id: "redo", label: "Redo", shortcut: "Ctrl+Shift+Z", disabled: !editor.state.canRedo, run: () => void editor.redo() },
        { id: "split", label: "Split at playhead", shortcut: "S", disabled: !clip, run: () => void editor.splitSelected() },
        { id: "delete", label: "Delete", shortcut: "Delete", disabled: !clip, run: () => void editor.deleteSelected() },
        { id: "unlink", label: "Unlink audio and video", disabled: !clip?.linkId, run: () => void editor.unlinkSelected() },
      ],
    },
    {
      id: "view",
      label: "View",
      items: [
        { id: "zin", label: "Zoom timeline in", shortcut: "Ctrl+=", run: () => editor.setPps((value) => Math.min(400, value + 20)) },
        { id: "zout", label: "Zoom timeline out", shortcut: "Ctrl+-", run: () => editor.setPps((value) => Math.max(20, value - 20)) },
        { id: "snap", label: editor.snap ? "Turn snap off" : "Turn snap on", shortcut: "N", run: () => editor.setSnap(!editor.snap) },
      ],
    },
    {
      id: "playback",
      label: "Playback",
      items: [
        { id: "play", label: editor.playing ? "Pause" : "Play", shortcut: "Space", run: () => void editor.togglePlay() },
        { id: "back", label: "Step back one frame", shortcut: "Left", run: () => editor.step(-1) },
        { id: "fwd", label: "Step forward one frame", shortcut: "Right", run: () => editor.step(1) },
        { id: "start", label: "Go to start", shortcut: "Home", run: () => editor.seek(0) },
        { id: "end", label: "Go to end", shortcut: "End", run: () => editor.seek(duration(editor)) },
      ],
    },
    {
      id: "timeline",
      label: "Timeline",
      items: [
        { id: "v", label: "Add video track", run: () => void editor.dispatch({ type: "addTrack", kind: "video" }) },
        { id: "a", label: "Add audio track", run: () => void editor.dispatch({ type: "addTrack", kind: "audio" }) },
      ],
    },
    {
      id: "effects",
      label: "Effects",
      items: [
        {
          id: "planned",
          label: "Effects are not in this build",
          run: () => document.dispatchEvent(new CustomEvent("pulsecut-help", { detail: "status" })),
        },
      ],
    },
    {
      id: "window",
      label: "Window",
      items: [{ id: "reset", label: "Reset layout", run: () => editor.resetLayout() }],
    },
    {
      id: "help",
      label: "Help",
      items: [
        { id: "keys", label: "Keyboard shortcuts", run: () => document.dispatchEvent(new CustomEvent("pulsecut-help", { detail: "keys" })) },
        { id: "status", label: "Feature status", run: () => document.dispatchEvent(new CustomEvent("pulsecut-help", { detail: "status" })) },
        { id: "about", label: "About PulseCut", run: () => document.dispatchEvent(new CustomEvent("pulsecut-help", { detail: "about" })) },
      ],
    },
  ];

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (!barRef.current?.contains(event.target as Node)) setOpen(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, []);

  return (
    <header className="menu-bar" ref={barRef} role="menubar">
      <div className="brand" aria-label="PulseCut">
        <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
          <rect width="18" height="18" rx="2" fill="#101218" />
          <rect x="3" y="8" width="12" height="2" fill="#3b82f6" />
          <rect x="8" y="3" width="2" height="12" fill="#e8eaef" />
        </svg>
        PulseCut
      </div>
      {menus.map((menu) => (
        <div className="menu" key={menu.id}>
          <button
            className="btn"
            role="menuitem"
            aria-expanded={open === menu.id}
            aria-haspopup="menu"
            onClick={() => setOpen((current) => (current === menu.id ? null : menu.id))}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown" || event.key === "Enter") {
                event.preventDefault();
                setOpen(menu.id);
              }
            }}
          >
            {menu.label}
          </button>
          {open === menu.id && (
            <div className="menu-pop" role="menu">
              {menu.items.map((item) => (
                <button
                  key={item.id}
                  className="menu-item"
                  role="menuitem"
                  disabled={item.disabled}
                  onClick={() => {
                    setOpen(null);
                    item.run();
                  }}
                >
                  <span>{item.label}</span>
                  {item.shortcut ? <kbd>{item.shortcut}</kbd> : null}
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
      <div className="ml-auto flex items-center gap-1">
        <button className="icon-btn" aria-label="Undo" disabled={!editor.state.canUndo} onClick={() => void editor.undo()}>
          Undo
        </button>
        <button className="icon-btn" aria-label="Redo" disabled={!editor.state.canRedo} onClick={() => void editor.redo()}>
          Redo
        </button>
        <button className="btn primary" onClick={() => editor.setExportOpen(true)}>
          Export
        </button>
      </div>
    </header>
  );
}

function duration(editor: ReturnType<typeof useEditor>) {
  return editor.state.project.sequence.tracks.reduce((end, track) => {
    return track.clips.reduce((inner, clip) => Math.max(inner, clip.timelineStartUs + clip.durationUs), end);
  }, 0);
}
