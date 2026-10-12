import { useEffect, useRef, useState } from "react";
import { ACTIONS, MENUS } from "../lib/actions";
import { useStore } from "../lib/store";
import logo from "../logo.svg";
import { IconExport, IconImport, IconRedo, IconUndo } from "./Icons";
import { Button, IconButton } from "./ui";

export function TopBar() {
  const [open, setOpen] = useState<number | null>(null);
  const [focusIdx, setFocusIdx] = useState(0);
  const barRef = useRef<HTMLDivElement>(null);
  const ps = useStore((s) => s.ps);
  const engine = useStore((s) => s.engine);
  useStore((s) => s.selection);
  useStore((s) => s.selectedAsset);

  useEffect(() => {
    if (open === null) return;
    const close = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) setOpen(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  useEffect(() => {
    if (open !== null) barRef.current?.querySelector<HTMLElement>(`[data-menu-item="${open}-${focusIdx}"]`)?.focus();
  }, [open, focusIdx]);

  const items = (i: number) => MENUS[i].items;
  const selectable = (i: number) =>
    items(i)
      .map((it, idx) => ({ it, idx }))
      .filter(({ it }) => typeof it === "string" && it !== "-")
      .map(({ idx }) => idx);

  const run = async (id: string) => {
    setOpen(null);
    const a = ACTIONS[id];
    if (a && (a.enabled?.() ?? true)) await a.run();
  };

  const title = ps ? `${ps.project.name}${ps.history.dirty ? " •" : ""}` : "";

  return (
    <header className="flex h-9 shrink-0 items-center gap-2 border-b border-pc-border bg-pc-panel px-2" ref={barRef}>
      <div className="flex items-center gap-1.5 pr-2">
        <img src={logo} alt="" className="h-5 w-5" />
        <span className="text-[13px] font-semibold tracking-wide">PulseCut</span>
      </div>
      <nav role="menubar" aria-label="Main menu" className="flex items-center">
        {MENUS.map((m, i) => (
          <div key={m.label} className="relative">
            <button
              type="button"
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={open === i}
              className={`h-7 rounded-[var(--radius-pc)] px-2.5 text-[12.5px] text-pc-muted hover:bg-pc-raised hover:text-pc-text ${open === i ? "bg-pc-raised text-pc-text" : ""}`}
              onMouseDown={(e) => {
                e.preventDefault();
                setOpen(open === i ? null : i);
                setFocusIdx(selectable(i)[0] ?? 0);
              }}
              onMouseEnter={() => open !== null && open !== i && (setOpen(i), setFocusIdx(selectable(i)[0] ?? 0))}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
                  e.preventDefault();
                  e.stopPropagation();
                  setOpen(i);
                  setFocusIdx(selectable(i)[0] ?? 0);
                }
              }}
            >
              {m.label}
            </button>
            {open === i && (
              <div
                role="menu"
                aria-label={m.label}
                className="absolute top-8 left-0 z-50 min-w-64 rounded-[4px] border border-pc-border-strong bg-pc-panel-2 py-1 shadow-xl"
                onKeyDown={(e) => {
                  e.stopPropagation();
                  const sel = selectable(i);
                  const pos = sel.indexOf(focusIdx);
                  if (e.key === "ArrowDown") setFocusIdx(sel[(pos + 1) % sel.length]);
                  else if (e.key === "ArrowUp") setFocusIdx(sel[(pos - 1 + sel.length) % sel.length]);
                  else if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
                    const n = (i + (e.key === "ArrowRight" ? 1 : MENUS.length - 1)) % MENUS.length;
                    setOpen(n);
                    setFocusIdx(selectable(n)[0] ?? 0);
                  } else if (e.key === "Escape") setOpen(null);
                  else return;
                  e.preventDefault();
                }}
              >
                {m.items.map((it, idx) => {
                  if (it === "-") return <div key={idx} className="my-1 h-px bg-pc-border" role="separator" />;
                  if (typeof it !== "string")
                    return (
                      <div key={idx} role="menuitem" aria-disabled="true" className="flex h-7 items-center px-3 text-[12px] text-pc-faint italic">
                        {it.planned}
                      </div>
                    );
                  const a = ACTIONS[it];
                  const enabled = a.enabled?.() ?? true;
                  let label = a.label;
                  if (it === "edit.undo" && ps?.history.undoLabel) label = `Undo ${ps.history.undoLabel}`;
                  if (it === "edit.redo" && ps?.history.redoLabel) label = `Redo ${ps.history.redoLabel}`;
                  return (
                    <button
                      key={idx}
                      type="button"
                      role="menuitem"
                      data-menu-item={`${i}-${idx}`}
                      disabled={!enabled}
                      tabIndex={-1}
                      className="flex h-7 w-full items-center justify-between gap-6 px-3 text-left text-[12.5px] text-pc-text hover:bg-pc-accent/25 focus:bg-pc-accent/25 focus:outline-none disabled:text-pc-faint"
                      onMouseEnter={() => setFocusIdx(idx)}
                      onClick={() => run(it)}
                    >
                      <span>{label}</span>
                      {a.keys && <span className="tabular text-[11px] text-pc-faint">{a.keys[0]}</span>}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        ))}
      </nav>
      <div className="min-w-0 flex-1 truncate text-center text-[12px] text-pc-muted" title={ps?.path ?? "Unsaved project"}>
        {title}
      </div>
      <div className="flex items-center gap-1">
        <IconButton label={`Undo${ps?.history.undoLabel ? ` ${ps.history.undoLabel}` : ""} (Ctrl+Z)`} disabled={!ps?.history.canUndo} onClick={() => ACTIONS["edit.undo"].run()}>
          <IconUndo />
        </IconButton>
        <IconButton label={`Redo${ps?.history.redoLabel ? ` ${ps.history.redoLabel}` : ""} (Ctrl+Shift+Z)`} disabled={!ps?.history.canRedo} onClick={() => ACTIONS["edit.redo"].run()}>
          <IconRedo />
        </IconButton>
        <div className="mx-1 h-5 w-px bg-pc-border" />
        <Button variant="ghost" disabled={!engine?.ok} onClick={() => ACTIONS["file.import"].run()}>
          <IconImport size={14} /> Import
        </Button>
        <Button variant="primary" disabled={!(ACTIONS["file.export"].enabled?.() ?? true)} onClick={() => ACTIONS["file.export"].run()}>
          <IconExport size={14} /> Export
        </Button>
      </div>
    </header>
  );
}
