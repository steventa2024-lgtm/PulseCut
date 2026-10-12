import { useEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from "react";

export function IconButton({
  label,
  active,
  className = "",
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; active?: boolean; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      className={`inline-flex h-7 min-w-7 items-center justify-center rounded-[var(--radius-pc)] px-1.5 text-pc-muted transition-colors hover:bg-pc-raised hover:text-pc-text disabled:pointer-events-none disabled:opacity-35 ${
        active ? "bg-pc-accent-lo/60 text-pc-accent-hi" : ""
      } ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

export function Button({
  variant = "default",
  className = "",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "default" | "primary" | "danger" | "ghost" }) {
  const styles = {
    default: "bg-pc-raised border border-pc-border-strong hover:border-pc-faint text-pc-text",
    primary: "bg-pc-accent hover:bg-pc-accent-hi text-white border border-transparent",
    danger: "bg-pc-danger/15 border border-pc-danger/50 text-pc-danger hover:bg-pc-danger/25",
    ghost: "border border-transparent hover:bg-pc-raised text-pc-muted hover:text-pc-text",
  }[variant];
  return (
    <button
      type="button"
      className={`inline-flex h-7 items-center gap-1.5 rounded-[var(--radius-pc)] px-3 text-[12px] font-medium transition-colors disabled:pointer-events-none disabled:opacity-40 ${styles} ${className}`}
      {...rest}
    />
  );
}

/** Numeric field that commits on Enter/blur, plus optional range slider that commits on release. */
export function NumberField({
  label,
  value,
  min,
  max,
  step = 1,
  unit,
  disabled,
  onCommit,
  onReset,
  isDefault,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  disabled?: boolean;
  onCommit: (v: number) => void;
  onReset?: () => void;
  isDefault?: boolean;
}) {
  const [text, setText] = useState(String(value));
  const [slider, setSlider] = useState(value);
  const editing = useRef(false);
  useEffect(() => {
    if (!editing.current) setText(String(round(value, step)));
    setSlider(value);
  }, [value, step]);
  const commit = (raw: string | number) => {
    const v = typeof raw === "number" ? raw : Number(raw);
    if (!Number.isFinite(v)) return setText(String(value));
    const c = Math.min(max, Math.max(min, v));
    setText(String(round(c, step)));
    if (c !== value) onCommit(c);
  };
  const id = `nf-${label.replace(/\s+/g, "-").toLowerCase()}`;
  return (
    <div className="grid grid-cols-[78px_1fr_64px_22px] items-center gap-2 py-1">
      <label htmlFor={id} className="truncate text-[12px] text-pc-muted">
        {label}
      </label>
      <input
        type="range"
        aria-label={`${label} slider`}
        min={min}
        max={max}
        step={step}
        value={slider}
        disabled={disabled}
        onChange={(e) => setSlider(Number(e.target.value))}
        onPointerUp={() => commit(slider)}
        onKeyUp={(e) => {
          if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(e.key)) commit(slider);
        }}
        className="h-1 w-full cursor-pointer accent-[var(--color-pc-accent)] disabled:opacity-40"
      />
      <div className="relative">
        <input
          id={id}
          className="tabular h-6 w-full rounded-[var(--radius-pc)] border border-pc-border bg-pc-bg px-1.5 text-right text-[12px] focus:border-pc-accent focus:outline-none disabled:opacity-40"
          value={text}
          disabled={disabled}
          inputMode="decimal"
          onFocus={() => (editing.current = true)}
          onChange={(e) => setText(e.target.value)}
          onBlur={(e) => {
            editing.current = false;
            commit(e.target.value);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur();
            if (e.key === "Escape") {
              setText(String(value));
              (e.target as HTMLInputElement).blur();
            }
            e.stopPropagation();
          }}
        />
        {unit && <span className="pointer-events-none absolute top-1/2 -right-0 hidden -translate-y-1/2 text-[10px] text-pc-faint">{unit}</span>}
      </div>
      {onReset ? (
        <button
          type="button"
          title={`Reset ${label}`}
          aria-label={`Reset ${label}`}
          disabled={disabled || isDefault}
          onClick={onReset}
          className="flex h-5 w-5 items-center justify-center rounded text-pc-faint hover:text-pc-text disabled:opacity-25"
        >
          ↺
        </button>
      ) : (
        <span />
      )}
    </div>
  );
}

function round(v: number, step: number) {
  const d = step >= 1 ? 0 : Math.min(4, Math.ceil(-Math.log10(step)));
  return Number(v.toFixed(d));
}

export function Modal({ title, onClose, children, width = 520 }: { title: string; onClose: () => void; children: ReactNode; width?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>("input,select,button")?.focus();
    return () => prev?.focus();
  }, []);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        style={{ width }}
        className="max-h-[90vh] overflow-auto rounded-[5px] border border-pc-border-strong bg-pc-panel shadow-2xl"
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
          e.stopPropagation();
        }}
      >
        <div className="flex h-10 items-center justify-between border-b border-pc-border px-4">
          <h2 className="text-[13px] font-semibold">{title}</h2>
          <button type="button" aria-label="Close" className="text-pc-muted hover:text-pc-text" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="p-4">{children}</div>
      </div>
    </div>
  );
}

export function PanelHeader({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="flex h-8 shrink-0 items-center justify-between border-b border-pc-border bg-pc-panel-2 px-2.5">
      <span className="text-[11px] font-semibold tracking-wider text-pc-muted uppercase">{title}</span>
      <div className="flex items-center gap-0.5">{children}</div>
    </div>
  );
}
