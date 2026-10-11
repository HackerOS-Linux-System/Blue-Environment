export interface Rgba { r: number; g: number; b: number; a: number }

const clamp255 = (n: number) => Math.min(255, Math.max(0, Math.round(n)));
const clamp01 = (n: number) => Math.min(1, Math.max(0, n));

/** Parses `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()` and `rgba()`; anything else → `null`. */
export function parseColor(input: string | undefined | null): Rgba | null {
  if (typeof input !== 'string') return null;
  const s = input.trim().toLowerCase();
  if (!s) return null;

  const hex = /^#([0-9a-f]{3,8})$/.exec(s);
  if (hex) {
    const h = hex[1];
    if (h.length === 3 || h.length === 4) {
      const [r, g, b, a] = h.split('').map((c) => parseInt(c + c, 16));
      return { r, g, b, a: h.length === 4 ? a / 255 : 1 };
    }
    if (h.length === 6 || h.length === 8) {
      return {
        r: parseInt(h.slice(0, 2), 16), g: parseInt(h.slice(2, 4), 16), b: parseInt(h.slice(4, 6), 16),
        a: h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1,
      };
    }
    return null;
  }

  const fn = /^rgba?\(\s*([^)]+)\)$/.exec(s);
  if (fn) {
    const parts = fn[1].split(/[\s,/]+/).filter(Boolean);
    if (parts.length < 3) return null;
    const ch = (p: string) => (p.endsWith('%') ? (parseFloat(p) / 100) * 255 : parseFloat(p));
    const r = ch(parts[0]); const g = ch(parts[1]); const b = ch(parts[2]);
    if ([r, g, b].some((n) => !Number.isFinite(n))) return null;
    let a = 1;
    if (parts[3] !== undefined) {
      a = parts[3].endsWith('%') ? parseFloat(parts[3]) / 100 : parseFloat(parts[3]);
      if (!Number.isFinite(a)) return null;
    }
    return { r: clamp255(r), g: clamp255(g), b: clamp255(b), a: clamp01(a) };
  }
  return null;
}

/** `"15, 23, 42"` — for `rgba(var(--panel-rgb), 0.9)`. `null` if unparsable. */
export function rgbTriplet(color: string | undefined | null): string | null {
  const c = parseColor(color);
  return c ? `${c.r}, ${c.g}, ${c.b}` : null;
}

/** `rgba(r, g, b, alpha)` for the colour (its own alpha is replaced). `null` if unparsable. */
export function withAlpha(color: string | undefined | null, alpha: number): string | null {
  const c = parseColor(color);
  return c ? `rgba(${c.r}, ${c.g}, ${c.b}, ${Math.round(clamp01(alpha) * 1000) / 1000})` : null;
}

/** Perceived luminance 0..1 (sRGB, Rec. 709 weights). `null` if unparsable. */
export function luminance(color: string | undefined | null): number | null {
  const c = parseColor(color);
  return c ? (0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b) / 255 : null;
}
