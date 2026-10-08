export interface Media { path: string; name: string; duration: number; width: number; height: number; hasAudio: boolean; hasVideo: boolean; thumb?: string }
export interface TimelineClip { id: number; media: Media; start: number; duration: number; speed: number; volume: number; fadeIn: number; fadeOut: number }
export interface TextItem { id: number; text: string; start: number; end: number; x: number; y: number; size: number; color: string }

let seq = 1;
export const nextId = () => seq++;

export const clipLength = (c: TimelineClip) => c.duration / c.speed;
export const timelineLength = (clips: TimelineClip[]) => clips.reduce((s, c) => s + clipLength(c), 0);

export function makeClip(media: Media): TimelineClip {
  return { id: nextId(), media, start: 0, duration: Math.max(0.1, media.duration || 5), speed: 1, volume: 1, fadeIn: 0, fadeOut: 0 };
}

/** Podział klipu w punkcie `t` (s na osi wynikowej); null gdy punkt nie jest wewnątrz klipu. */
export function splitAt(clips: TimelineClip[], t: number): TimelineClip[] | null {
  let acc = 0;
  for (let i = 0; i < clips.length; i++) {
    const c = clips[i], len = clipLength(c);
    if (t > acc + 0.05 && t < acc + len - 0.05) {
      const srcOffset = (t - acc) * c.speed;
      const a: TimelineClip = { ...c, duration: srcOffset, fadeOut: 0 };
      const b: TimelineClip = { ...c, id: nextId(), start: c.start + srcOffset, duration: c.duration - srcOffset, fadeIn: 0 };
      return [...clips.slice(0, i), a, b, ...clips.slice(i + 1)];
    }
    acc += len;
  }
  return null;
}

export function moveClip(clips: TimelineClip[], from: number, to: number): TimelineClip[] {
  if (from === to || from < 0 || to < 0 || from >= clips.length || to >= clips.length) return clips;
  const out = clips.slice();
  const [c] = out.splice(from, 1);
  out.splice(to, 0, c);
  return out;
}

export const fmtTime = (s: number) => {
  const m = Math.floor(s / 60), r = s - m * 60;
  return `${m}:${r.toFixed(1).padStart(4, '0')}`;
};

export function toExportRequest(clips: TimelineClip[], texts: TextItem[], music: { path: string; volume: number } | null,
  opts: { width: number; height: number; fps: number; crf: number; output: string }) {
  return {
    clips: clips.map((c) => ({ path: c.media.path, start: c.start, duration: c.duration, speed: c.speed, volume: c.volume, fadeIn: c.fadeIn, fadeOut: c.fadeOut, hasAudio: c.media.hasAudio })),
    texts: texts.map(({ text, start, end, x, y, size, color }) => ({ text, start, end, x, y, size, color })),
    music, ...opts,
  };
}
