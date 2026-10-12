import { useEffect, useState } from "react";
import { ipc } from "./ipc";
import type { Uuid } from "./types";

const thumbs = new Map<Uuid, Promise<string | null>>();
const waves = new Map<Uuid, Promise<{ peaksPerSecond: number; peaks: Float32Array } | null>>();

export function useThumbnail(assetId: Uuid | null, enabled = true): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!assetId || !enabled) return setUrl(null);
    let p = thumbs.get(assetId);
    if (!p) {
      p = ipc
        .assetThumbnail(assetId)
        .then((buf) => URL.createObjectURL(new Blob([buf], { type: "image/jpeg" })))
        .catch(() => null);
      thumbs.set(assetId, p);
    }
    let live = true;
    void p.then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, [assetId, enabled]);
  return url;
}

export function useWaveform(assetId: Uuid | null, enabled = true) {
  const [w, setW] = useState<{ peaksPerSecond: number; peaks: Float32Array } | null>(null);
  useEffect(() => {
    if (!assetId || !enabled) return setW(null);
    let p = waves.get(assetId);
    if (!p) {
      p = ipc
        .assetWaveform(assetId)
        .then((r) => ({ peaksPerSecond: r.peaksPerSecond, peaks: Float32Array.from(r.peaks) }))
        .catch(() => null);
      waves.set(assetId, p);
    }
    let live = true;
    void p.then((v) => live && setW(v));
    return () => {
      live = false;
    };
  }, [assetId, enabled]);
  return w;
}

/** Drop cached analysis after relinking (the file behind the asset changed). */
export function invalidateMedia(assetId: Uuid) {
  thumbs.delete(assetId);
  waves.delete(assetId);
}
