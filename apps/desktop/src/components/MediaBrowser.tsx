import { useEffect, useState } from "react";
import { api } from "../api";
import { formatTimecode } from "../lib/time";
import { useEditor } from "../state/EditorContext";
import type { Asset } from "../types";

const thumbs = new Map<string, string | null>();

export function MediaBrowser() {
  const editor = useEditor();
  const [query, setQuery] = useState("");
  const assets = editor.state.project.assets.filter((asset) =>
    asset.name.toLowerCase().includes(query.trim().toLowerCase()),
  );
  return (
    <section className="panel" aria-label="Project media">
      <div className="browser-head">
        <strong className="text-ink">Media</strong>
        <button className="btn" onClick={() => void editor.importMedia()}>
          Import
        </button>
      </div>
      <div className="p-2">
        <input
          className="field"
          value={query}
          placeholder="Search media"
          aria-label="Search media"
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>
      <div className="scroll flex-1 px-1 pb-2">
        {assets.length === 0 ? (
          <div className="px-2 py-6 text-center text-sm text-faint">
            <p>No media in this project.</p>
            <button className="btn mt-3" onClick={() => void editor.importMedia()}>
              Import media
            </button>
            <button className="btn mt-2" onClick={() => void editor.loadSample()}>
              Load sample timeline
            </button>
          </div>
        ) : (
          assets.map((asset) => (
            <AssetRow key={asset.id} asset={asset} />
          ))
        )}
      </div>
    </section>
  );
}

function AssetRow({ asset }: { asset: Asset }) {
  const editor = useEditor();
  const [thumb, setThumb] = useState<string | null | undefined>(thumbs.get(asset.id));
  useEffect(() => {
    if (!asset.hasVideo || asset.offline || thumbs.has(asset.id)) return;
    let cancelled = false;
    void api.thumbnail(asset.path, Math.min(500_000, Math.floor(asset.durationUs / 2))).then(
      (data) => {
        thumbs.set(asset.id, data);
        if (!cancelled) setThumb(data);
      },
      () => {
        thumbs.set(asset.id, null);
        if (!cancelled) setThumb(null);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [asset]);
  const fps = asset.fpsNum && asset.fpsDen ? `${asset.fpsNum}/${asset.fpsDen}` : "audio";
  return (
    <button
      className="asset"
      draggable={!asset.offline}
      onDragStart={(event) => {
        event.dataTransfer.setData("application/x-pulsecut-asset", asset.id);
        event.dataTransfer.effectAllowed = "copy";
      }}
      onDoubleClick={() => {
        const video = editor.state.project.sequence.tracks.find((track) => track.kind === "video" && !track.locked);
        const audio = editor.state.project.sequence.tracks.find((track) => track.kind === "audio" && !track.locked);
        const track = asset.hasVideo ? video : audio;
        if (!track) {
          editor.clearError();
          return;
        }
        const end = editor.state.project.sequence.tracks.reduce(
          (max, item) => item.clips.reduce((inner, clip) => Math.max(inner, clip.timelineStartUs + clip.durationUs), max),
          0,
        );
        void editor.placeAsset(asset, track, end);
      }}
      title={asset.path}
    >
      {thumb ? (
        <img className="thumb" alt="" src={`data:image/jpeg;base64,${thumb}`} />
      ) : (
        <div className="thumb missing">{asset.offline ? "Offline" : asset.hasVideo ? "…" : "Audio"}</div>
      )}
      <span className="min-w-0">
        <span className="block truncate text-ink">{asset.name}</span>
        <span className="block truncate text-[11px] text-faint">
          {formatTimecode(asset.durationUs, asset.fpsNum ?? 30, asset.fpsDen ?? 1)}
          {asset.width && asset.height ? ` · ${asset.width}×${asset.height}` : ""} · {fps}
          {asset.vfrSuspected ? " · VFR" : ""}
          {asset.hdr ? " · HDR" : ""}
          {asset.offline ? " · offline" : ""}
        </span>
      </span>
    </button>
  );
}
