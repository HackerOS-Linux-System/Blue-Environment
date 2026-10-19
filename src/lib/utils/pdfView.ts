export const ZOOM_STEPS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4] as const;
export const BASE_DPI = 96; // zoom 1 = 96 dpi = 1 CSS px per PDF point * 96/72
export const MIN_ZOOM = ZOOM_STEPS[0];
export const MAX_ZOOM = ZOOM_STEPS[ZOOM_STEPS.length - 1];

export function clampZoom(z: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}

export function zoomIn(z: number): number {
  return ZOOM_STEPS.find((s) => s > z + 1e-9) ?? MAX_ZOOM;
}

export function zoomOut(z: number): number {
  return [...ZOOM_STEPS].reverse().find((s) => s < z - 1e-9) ?? MIN_ZOOM;
}

/** Zoom that makes a page `pageWidthPts` wide fill `containerPx` (minus padding). */
export function fitWidthZoom(containerPx: number, pageWidthPts: number, padding = 32): number {
  if (pageWidthPts <= 0 || containerPx <= padding) return 1;
  const pagePxAtZoom1 = (pageWidthPts * BASE_DPI) / 72;
  return clampZoom(Math.round(((containerPx - padding) / pagePxAtZoom1) * 100) / 100);
}

export function fitPageZoom(containerW: number, containerH: number, pageWpts: number, pageHpts: number, padding = 32): number {
  if (pageHpts <= 0) return 1;
  const byH = ((containerH - padding) * 72) / (pageHpts * BASE_DPI);
  return Math.min(fitWidthZoom(containerW, pageWpts, padding), clampZoom(Math.round(byH * 100) / 100));
}

/** Rendering resolution handed to the backend for a zoom level, honouring HiDPI. */
export function dpiFor(zoom: number, devicePixelRatio = 1): number {
  return Math.round(Math.min(300, Math.max(30, BASE_DPI * zoom * Math.max(1, devicePixelRatio))));
}

export function pageSizePx(pageWpts: number, pageHpts: number, zoom: number): { w: number; h: number } {
  const k = (BASE_DPI / 72) * zoom;
  return { w: Math.round(pageWpts * k), h: Math.round(pageHpts * k) };
}

export function clampPage(p: number, pages: number): number {
  if (!Number.isFinite(p) || pages < 1) return 1;
  return Math.min(pages, Math.max(1, Math.trunc(p)));
}

/** Parses what a person types into the "go to page" box. */
export function parsePageInput(raw: string, pages: number): number | null {
  const n = Number(raw.trim());
  if (!raw.trim() || !Number.isFinite(n)) return null;
  return clampPage(n, pages);
}

/** Pages whose boxes intersect the viewport (with a margin), for lazy rendering. */
export function visiblePages(tops: number[], heights: number[], scrollTop: number, viewH: number, margin = 600): number[] {
  const out: number[] = [];
  for (let i = 0; i < tops.length; i++) {
    const top = tops[i];
    const bottom = top + (heights[i] ?? 0);
    if (bottom >= scrollTop - margin && top <= scrollTop + viewH + margin) out.push(i + 1);
  }
  return out;
}

/** The page that occupies most of the viewport centre (for the "page N of M" display). */
export function currentPage(tops: number[], heights: number[], scrollTop: number, viewH: number): number {
  const mid = scrollTop + viewH / 2;
  for (let i = 0; i < tops.length; i++) {
    if (mid >= tops[i] && mid <= tops[i] + (heights[i] ?? 0) + 12) return i + 1;
  }
  return tops.length ? (mid < tops[0] ? 1 : tops.length) : 1;
}
