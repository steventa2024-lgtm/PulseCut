import { useMemo, useState } from "react";
import { useStore } from "../lib/store";
import { invalidateMedia, useThumbnail } from "../lib/mediaCache";
import { formatBytes, formatDuration, formatFps, toNumber } from "../lib/time";
import type { Asset } from "../lib/types";
import { IconFilm, IconImage, IconImport, IconSearch, IconTrash, IconWarning, IconWave } from "./Icons";
import { Button, IconButton, PanelHeader } from "./ui";

export const ASSET_MIME = "application/x-pulsecut-asset";

function AssetRow({ asset, offline, selected }: { asset: Asset; offline: boolean; selected: boolean }) {
  const thumb = useThumbnail(asset.id, !!asset.media.video && !offline);
  const select = useStore((s) => s.selectAsset);
  const addAtPlayhead = useStore((s) => s.addAssetAtPlayhead);
  const v = asset.media.video;
  const Icon = asset.media.kind === "audio" ? IconWave : asset.media.kind === "image" ? IconImage : IconFilm;
  return (
    <li
      role="option"
      aria-selected={selected}
      tabIndex={0}
      draggable={!offline}
      onDragStart={(e) => {
        e.dataTransfer.setData(ASSET_MIME, asset.id);
        e.dataTransfer.effectAllowed = "copy";
        select(asset.id);
      }}
      onClick={() => select(asset.id)}
      onDoubleClick={() => !offline && addAtPlayhead(asset.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !offline) {
          e.stopPropagation();
          void addAtPlayhead(asset.id);
        }
      }}
      className={`flex cursor-grab items-center gap-2 rounded-[var(--radius-pc)] border px-1.5 py-1.5 ${
        selected ? "border-pc-accent/70 bg-pc-accent/15" : "border-transparent hover:bg-pc-raised"
      }`}
      title={`${asset.path}\nDouble-click or Enter to add at the playhead; drag to a track.`}
    >
      <div className="relative flex h-[34px] w-[60px] shrink-0 items-center justify-center overflow-hidden rounded-[2px] bg-pc-bg">
        {thumb ? <img src={thumb} alt="" className="h-full w-full object-cover" draggable={false} /> : <Icon size={18} className="text-pc-faint" />}
        {offline && (
          <div className="absolute inset-0 flex items-center justify-center bg-pc-danger/40">
            <IconWarning size={16} />
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className={`truncate text-[12.5px] ${offline ? "text-pc-danger" : ""}`}>{asset.name}</div>
        <div className="tabular truncate text-[10.5px] text-pc-faint">
          {offline
            ? "Media offline"
            : [
                asset.media.kind !== "image" ? formatDuration(toNumber(asset.media.duration)) : "still",
                v ? `${v.width}×${v.height}` : null,
                v && asset.media.kind !== "image" ? `${formatFps(v.frameRate)} fps` : null,
                asset.media.audio ? "♪" : null,
              ]
                .filter(Boolean)
                .join(" · ")}
        </div>
      </div>
    </li>
  );
}

export function MediaBrowser() {
  const ps = useStore((s) => s.ps);
  const selectedAsset = useStore((s) => s.selectedAsset);
  const importFiles = useStore((s) => s.importFiles);
  const relink = useStore((s) => s.relink);
  const removeAsset = useStore((s) => s.removeAsset);
  const engineOk = useStore((s) => !!s.engine?.ok);
  const busy = useStore((s) => s.busy);
  const [query, setQuery] = useState("");
  const offline = useMemo(() => new Set(ps?.offline ?? []), [ps?.offline]);
  const assets = ps?.project.assets ?? [];
  const filtered = assets.filter((a) => a.name.toLowerCase().includes(query.trim().toLowerCase()));
  const sel = assets.find((a) => a.id === selectedAsset) ?? null;

  return (
    <section className="flex h-full min-h-0 flex-col bg-pc-panel" aria-label="Media browser">
      <PanelHeader title="Project Media">
        <IconButton label="Import media (Ctrl+I)" disabled={!engineOk} onClick={() => importFiles()}>
          <IconImport />
        </IconButton>
      </PanelHeader>
      <div className="p-2">
        <label className="flex h-7 items-center gap-1.5 rounded-[var(--radius-pc)] border border-pc-border bg-pc-bg px-2 focus-within:border-pc-accent">
          <IconSearch size={13} className="text-pc-faint" />
          <input
            aria-label="Search media"
            placeholder="Search media"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.stopPropagation()}
            className="w-full bg-transparent text-[12px] outline-none placeholder:text-pc-faint"
          />
        </label>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-2">
        {assets.length === 0 ? (
          <div className="mt-6 flex flex-col items-center gap-3 px-4 text-center text-[12px] text-pc-muted">
            <p>Import video, audio or image files, or drop them onto the window.</p>
            <Button variant="primary" disabled={!engineOk} onClick={() => importFiles()}>
              <IconImport size={14} /> Import Media
            </Button>
          </div>
        ) : (
          <ul role="listbox" aria-label="Media files" className="flex flex-col gap-0.5 pb-2">
            {filtered.map((a) => (
              <AssetRow key={a.id} asset={a} offline={offline.has(a.id)} selected={a.id === selectedAsset} />
            ))}
            {filtered.length === 0 && <li className="py-4 text-center text-[12px] text-pc-faint">No media matches “{query}”.</li>}
          </ul>
        )}
        {busy && <div className="py-2 text-center text-[11px] text-pc-accent-hi">{busy}</div>}
      </div>
      {sel && (
        <div className="shrink-0 border-t border-pc-border bg-pc-panel-2 p-2.5 text-[11.5px]">
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <span className="truncate font-medium" title={sel.path}>
              {sel.name}
            </span>
            <div className="flex shrink-0 gap-1">
              {offline.has(sel.id) && (
                <Button
                  variant="danger"
                  className="h-6 px-2"
                  onClick={async () => {
                    invalidateMedia(sel.id);
                    await relink(sel.id);
                  }}
                >
                  Relink…
                </Button>
              )}
              <IconButton label="Remove from project" onClick={() => removeAsset(sel.id)}>
                <IconTrash size={14} />
              </IconButton>
            </div>
          </div>
          <dl className="tabular grid grid-cols-[84px_1fr] gap-x-2 gap-y-0.5 text-pc-muted">
            <dt>Type</dt>
            <dd className="text-pc-text">{sel.media.kind}</dd>
            <dt>Container</dt>
            <dd className="truncate text-pc-text">{sel.media.container.split(",")[0]}</dd>
            {sel.media.kind !== "image" && (
              <>
                <dt>Duration</dt>
                <dd className="text-pc-text">{toNumber(sel.media.duration).toFixed(3)} s</dd>
              </>
            )}
            {sel.media.video && (
              <>
                <dt>Video</dt>
                <dd className="text-pc-text">
                  {sel.media.video.codec} {sel.media.video.width}×{sel.media.video.height}
                  {sel.media.kind !== "image" && ` @ ${formatFps(sel.media.video.frameRate)} fps`}
                </dd>
              </>
            )}
            <dt>Audio</dt>
            <dd className="text-pc-text">
              {sel.media.audio ? `${sel.media.audio.codec} ${sel.media.audio.sampleRate} Hz ${sel.media.audio.channels} ch` : "none"}
              {sel.media.audioStreamCount > 1 && ` (+${sel.media.audioStreamCount - 1} more, first used)`}
            </dd>
            <dt>Size</dt>
            <dd className="text-pc-text">{formatBytes(sel.media.sizeBytes)}</dd>
          </dl>
          {sel.media.video?.variableFrameRate && (
            <p className="mt-1.5 text-pc-warn">Variable frame rate detected: frames are resampled to the sequence rate.</p>
          )}
          {sel.media.video?.hdr && <p className="mt-1.5 text-pc-warn">HDR source: preview and export are SDR; colours will not be tone-mapped.</p>}
          {!!sel.media.video?.rotation && <p className="mt-1.5 text-pc-muted">Rotated {sel.media.video.rotation}° (applied automatically).</p>}
        </div>
      )}
    </section>
  );
}
