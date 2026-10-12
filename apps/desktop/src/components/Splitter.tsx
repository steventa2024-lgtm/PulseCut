import { useRef } from "react";

/** Draggable 1px divider with a wider hit area. Keyboard: arrows resize by 16px. */
export function Splitter({
  direction,
  onDelta,
  onDone,
  label,
}: {
  direction: "vertical" | "horizontal";
  onDelta: (deltaPx: number) => void;
  onDone?: () => void;
  label: string;
}) {
  const last = useRef(0);
  const vertical = direction === "vertical";
  return (
    <div
      role="separator"
      aria-orientation={vertical ? "vertical" : "horizontal"}
      aria-label={label}
      tabIndex={0}
      className={`group relative z-10 shrink-0 bg-pc-border focus-visible:bg-pc-accent ${vertical ? "w-px cursor-col-resize" : "h-px cursor-row-resize"}`}
      onPointerDown={(e) => {
        (e.target as HTMLElement).setPointerCapture(e.pointerId);
        last.current = vertical ? e.clientX : e.clientY;
      }}
      onPointerMove={(e) => {
        if (!(e.target as HTMLElement).hasPointerCapture(e.pointerId)) return;
        const pos = vertical ? e.clientX : e.clientY;
        onDelta(pos - last.current);
        last.current = pos;
      }}
      onPointerUp={() => onDone?.()}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 64 : 16;
        const keys: Record<string, number> = vertical ? { ArrowLeft: -step, ArrowRight: step } : { ArrowUp: -step, ArrowDown: step };
        const d = keys[e.key];
        if (d) {
          onDelta(d);
          onDone?.();
          e.preventDefault();
          e.stopPropagation();
        }
      }}
    >
      <div className={`absolute ${vertical ? "inset-y-0 -left-[3px] w-[7px]" : "inset-x-0 -top-[3px] h-[7px]"} group-hover:bg-pc-accent/40`} />
    </div>
  );
}
