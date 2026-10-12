import { Channel } from "@tauri-apps/api/core";
import { ipc } from "./ipc";
import type { Sequence } from "./types";

// Real-time preview: Rust streams composited MJPEG frames (same render plan as export)
// and renders the audio mix to PCM in chunks. The Web Audio clock is the master; each
// display refresh shows the newest frame that is due and counts skipped frames as drops.

const AUDIO_CHUNK_SECONDS = 20;
const AUDIO_PREFETCH_SECONDS = 8;
const MAX_QUEUE = 90;

interface Callbacks {
  onTime: (frame: number) => void;
  onDropped: (n: number) => void;
  onEnd: (error: string | null) => void;
}

type FrameSink = (bitmap: ImageBitmap) => void;

function pcmToBuffer(ctx: AudioContext, pcm: ArrayBuffer, sampleRate: number): AudioBuffer | null {
  const samples = pcm.byteLength / 4;
  if (samples < 1) return null;
  const view = new DataView(pcm);
  const buf = ctx.createBuffer(2, samples, sampleRate);
  const l = buf.getChannelData(0);
  const r = buf.getChannelData(1);
  for (let i = 0; i < samples; i++) {
    l[i] = view.getInt16(i * 4, true) / 32768;
    r[i] = view.getInt16(i * 4 + 2, true) / 32768;
  }
  return buf;
}

class PlaybackController {
  private session = 0;
  private sink: FrameSink | null = null;
  private ctx: AudioContext | null = null;
  private sources: AudioBufferSourceNode[] = [];
  private raf = 0;
  private queue: { idx: number; bmp: ImageBitmap }[] = [];

  setFrameSink(sink: FrameSink | null) {
    this.sink = sink;
  }

  async stop() {
    this.session++;
    cancelAnimationFrame(this.raf);
    for (const s of this.sources) {
      try {
        s.stop();
      } catch {
        /* already stopped */
      }
    }
    this.sources = [];
    for (const f of this.queue) f.bmp.close();
    this.queue = [];
    await ipc.playbackStop();
  }

  async start(seq: Sequence, from: number, end: number, scale: number, cb: Callbacks) {
    await this.stop();
    const id = ++this.session;
    const live = () => id === this.session;
    const fps = seq.frameRate.num / seq.frameRate.den;
    let streamEnded = false;
    let streamError: string | null = null;
    let resolveFirst: () => void = () => {};
    const firstFrame = new Promise<void>((r) => (resolveFirst = r));
    let decode = Promise.resolve();

    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (buf) => {
      if (!live()) return;
      const idx = Number(new DataView(buf).getBigInt64(0, true));
      if (idx < 0) {
        if (idx === -2) streamError = new TextDecoder().decode(new Uint8Array(buf, 8)).trim().split("\n").pop() ?? "render error";
        streamEnded = true;
        resolveFirst();
        return;
      }
      const blob = new Blob([new Uint8Array(buf, 8)], { type: "image/jpeg" });
      decode = decode
        .then(() => createImageBitmap(blob))
        .then((bmp) => {
          if (!live()) return bmp.close();
          this.queue.push({ idx, bmp });
          resolveFirst();
          while (this.queue.length > MAX_QUEUE) this.queue.shift()!.bmp.close();
        })
        .catch(() => undefined);
    };

    const ctx = (this.ctx ??= new AudioContext());
    const chunkFrames = Math.round(AUDIO_CHUNK_SECONDS * fps);
    const loadChunk = (i: number) => {
      const s = from + i * chunkFrames;
      if (s >= end) return Promise.resolve(null);
      return ipc.renderAudio(s, Math.min(end, s + chunkFrames)).then((pcm) => pcmToBuffer(ctx, pcm, seq.sampleRate));
    };

    const firstAudio = loadChunk(0);
    await ipc.playbackStart(from, scale, channel);
    await Promise.race([Promise.all([firstFrame, firstAudio]), new Promise((r) => setTimeout(r, 4000))]);
    if (!live()) return;

    await Promise.race([ctx.resume(), new Promise((r) => setTimeout(r, 300))]);
    const audioClock = ctx.state === "running";
    const t0Audio = ctx.currentTime + 0.05;
    const t0Perf = performance.now() + 50;
    const now = () => (audioClock ? ctx.currentTime - t0Audio : (performance.now() - t0Perf) / 1000);

    const schedule = (buf: AudioBuffer | null, i: number) => {
      if (!buf || !live() || !audioClock) return;
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.connect(ctx.destination);
      const at = t0Audio + i * (chunkFrames / fps);
      const late = ctx.currentTime - at;
      if (late > 0) src.start(ctx.currentTime, late);
      else src.start(at);
      this.sources.push(src);
    };
    schedule(await firstAudio, 0);
    let nextChunk = 1;

    let dropped = 0;
    let lastReported = -1;
    const tick = () => {
      if (!live()) return;
      const t = now();
      const target = from + Math.max(0, Math.floor(t * fps));
      if (target >= end || (streamEnded && streamError)) {
        cb.onTime(Math.min(target, end - 1));
        void this.stop().then(() => cb.onEnd(streamError));
        return;
      }
      let shown: { idx: number; bmp: ImageBitmap } | null = null;
      while (this.queue.length && this.queue[0].idx <= target) {
        const f = this.queue.shift()!;
        if (shown) {
          shown.bmp.close();
          dropped++;
        }
        shown = f;
      }
      if (shown) {
        if (this.sink) this.sink(shown.bmp);
        else shown.bmp.close();
      }
      if (target !== lastReported) {
        lastReported = target;
        cb.onTime(target);
        cb.onDropped(dropped);
      }
      if (t > nextChunk * AUDIO_CHUNK_SECONDS - AUDIO_PREFETCH_SECONDS) {
        const i = nextChunk++;
        void loadChunk(i).then((b) => schedule(b, i));
      }
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }
}

export const playback = new PlaybackController();
