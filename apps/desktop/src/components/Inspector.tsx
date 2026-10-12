import { useStore } from "../lib/store";
import { formatFps, formatTimecode, toNumber } from "../lib/time";
import { DEFAULT_PARAMS, type Clip, type ClipParams, type Project, type Track } from "../lib/types";
import { IconLink } from "./Icons";
import { NumberField, PanelHeader } from "./ui";

function findClip(p: Project, id: string): { clip: Clip; track: Track } | null {
  for (const track of p.sequence.tracks) {
    const clip = track.clips.find((c) => c.id === id);
    if (clip) return { clip, track };
  }
  return null;
}

export function Inspector() {
  const ps = useStore((s) => s.ps);
  const selection = useStore((s) => s.selection);
  const edit = useStore((s) => s.edit);

  if (!ps) return <section className="h-full bg-pc-panel" />;
  const seq = ps.project.sequence;
  const found = selection.length === 1 ? findClip(ps.project, selection[0]) : null;

  const body = (() => {
    if (selection.length > 1) return <p className="p-3 text-[12px] text-pc-muted">{selection.length} clips selected. Select one clip to edit its properties.</p>;
    if (!found)
      return (
        <div className="p-3 text-[12px]">
          <h3 className="mb-2 text-[11px] font-semibold tracking-wider text-pc-muted uppercase">Sequence</h3>
          <dl className="tabular grid grid-cols-[96px_1fr] gap-y-1 text-pc-muted">
            <dt>Name</dt>
            <dd className="text-pc-text">{seq.name}</dd>
            <dt>Frame size</dt>
            <dd className="text-pc-text">
              {seq.width}×{seq.height}
            </dd>
            <dt>Frame rate</dt>
            <dd className="text-pc-text">{formatFps(seq.frameRate)} fps</dd>
            <dt>Audio</dt>
            <dd className="text-pc-text">{seq.sampleRate} Hz stereo</dd>
            <dt>Tracks</dt>
            <dd className="text-pc-text">
              {seq.tracks.filter((t) => t.kind === "video").length} video, {seq.tracks.filter((t) => t.kind === "audio").length} audio
            </dd>
          </dl>
          <p className="mt-4 text-pc-faint">Select a clip in the timeline to edit its properties.</p>
        </div>
      );
    const { clip, track } = found;
    const asset = ps.project.assets.find((a) => a.id === clip.assetId);
    const isVideo = track.kind === "video";
    const activeTab = isVideo ? "video" : "audio";
    const params = clip.params;
    const locked = track.locked;
    const set = (patch: Partial<ClipParams>) => void edit({ type: "setClipParams", clipId: clip.id, params: { ...params, ...patch } });
    const fps = seq.frameRate;
    return (
      <div className="text-[12px]">
        <div className="border-b border-pc-border p-3">
          <div className="mb-1 truncate font-medium" title={asset?.path}>
            {asset?.name ?? "Unknown media"}
          </div>
          <dl className="tabular grid grid-cols-[84px_1fr] gap-y-0.5 text-[11.5px] text-pc-muted">
            <dt>Track</dt>
            <dd className="text-pc-text">
              {track.name}
              {locked && " (locked)"}
            </dd>
            <dt>Start</dt>
            <dd className="text-pc-text">{formatTimecode(clip.start, fps)}</dd>
            <dt>Duration</dt>
            <dd className="text-pc-text">
              {formatTimecode(clip.duration, fps)} ({clip.duration} f)
            </dd>
            <dt>Source in</dt>
            <dd className="text-pc-text">{toNumber(clip.sourceIn).toFixed(3)} s</dd>
            <dt>Linked</dt>
            <dd className="flex items-center gap-1 text-pc-text">{clip.link ? <><IconLink size={12} /> with {isVideo ? "audio" : "video"}</> : "no"}</dd>
          </dl>
        </div>
        <div role="tablist" aria-label="Clip property groups" className="flex border-b border-pc-border px-2">
          {(["video", "audio"] as const).map((t) => (
            <button
              key={t}
              role="tab"
              type="button"
              aria-selected={activeTab === t}
              disabled={t !== activeTab}
              className={`h-8 px-3 text-[12px] capitalize ${activeTab === t ? "border-b-2 border-pc-accent text-pc-text" : "text-pc-faint"} disabled:cursor-default`}
              title={t !== activeTab ? `This is a ${track.kind} clip` : undefined}
            >
              {t === "video" ? "Motion & Opacity" : "Audio"}
            </button>
          ))}
        </div>
        <div className="p-3" role="tabpanel">
          {isVideo ? (
            <>
              <NumberField
                label="Scale %"
                value={Math.round(params.scale * 1000) / 10}
                min={1}
                max={1000}
                step={0.1}
                disabled={locked}
                onCommit={(v) => set({ scale: v / 100 })}
                onReset={() => set({ scale: DEFAULT_PARAMS.scale })}
                isDefault={params.scale === DEFAULT_PARAMS.scale}
              />
              <NumberField
                label="Position X"
                value={params.positionX}
                min={-seq.width * 2}
                max={seq.width * 2}
                step={1}
                disabled={locked}
                onCommit={(v) => set({ positionX: v })}
                onReset={() => set({ positionX: 0 })}
                isDefault={params.positionX === 0}
              />
              <NumberField
                label="Position Y"
                value={params.positionY}
                min={-seq.height * 2}
                max={seq.height * 2}
                step={1}
                disabled={locked}
                onCommit={(v) => set({ positionY: v })}
                onReset={() => set({ positionY: 0 })}
                isDefault={params.positionY === 0}
              />
              <NumberField
                label="Opacity %"
                value={Math.round(params.opacity * 1000) / 10}
                min={0}
                max={100}
                step={0.1}
                disabled={locked}
                onCommit={(v) => set({ opacity: v / 100 })}
                onReset={() => set({ opacity: 1 })}
                isDefault={params.opacity === 1}
              />
              <p className="mt-3 text-[11px] text-pc-faint">Position is the offset of the clip centre from the frame centre in sequence pixels. Keyframes are planned (Phase 2).</p>
            </>
          ) : (
            <>
              <NumberField
                label="Gain dB"
                value={params.gainDb}
                min={-60}
                max={24}
                step={0.1}
                disabled={locked}
                onCommit={(v) => set({ gainDb: v })}
                onReset={() => set({ gainDb: 0 })}
                isDefault={params.gainDb === 0}
              />
              <p className="mt-3 text-[11px] text-pc-faint">Fades, pan and keyframes are planned (Phase 2).</p>
            </>
          )}
        </div>
      </div>
    );
  })();

  return (
    <section className="flex h-full min-h-0 flex-col bg-pc-panel" aria-label="Inspector">
      <PanelHeader title="Inspector" />
      <div className="min-h-0 flex-1 overflow-auto">{body}</div>
    </section>
  );
}
