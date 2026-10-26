import widgetsStoreFallback from '../../../config/stores/widgets-store.json';
import { BUILTIN_WIDGET_TYPES, STORE_WIDGET_TYPES, WIDGET_TYPES, type WidgetType } from './widgetModel';

export type ConfigValue = string | number | boolean;

export interface CatalogEntry {
  id: string;
  name: string;
  description: string;
  author: string;
  /** One emoji (or a couple of characters). */
  icon: string;
  category: string;
  version: string;
  type: WidgetType;
  size?: { w: number; h: number };
  config?: Record<string, ConfigValue>;
  /** Accent colour for the card (`#rrggbb`). */
  accent?: string;
  tags?: string[];
  /** Ships with the shell: always installed, cannot be removed. */
  builtin?: boolean;
}

export interface CatalogResult {
  entries: CatalogEntry[];
  warnings: string[];
}

export const CATALOG_CATEGORIES = ['Time', 'Productivity', 'Information', 'Inspiration', 'System'] as const;

const ID_RE = /^[a-z0-9][a-z0-9-]{1,47}$/;
const MAX_CONFIG_KEYS = 12;
const MAX_STRING = 2000;

function str(v: unknown, max: number): string {
  return typeof v === 'string' ? v.trim().slice(0, max) : '';
}

function num(v: unknown, lo: number, hi: number): number | undefined {
  return typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, Math.round(v))) : undefined;
}

/** Validates one raw entry; returns the cleaned entry or a reason it was skipped. */
export function sanitizeEntry(raw: unknown): { entry?: CatalogEntry; warning?: string } {
  if (!raw || typeof raw !== 'object') return { warning: 'entry is not an object' };
  const o = raw as Record<string, unknown>;
  const id = str(o.id, 64).toLowerCase();
  if (!ID_RE.test(id)) return { warning: `"${str(o.id, 40)}": invalid id (use a-z, 0-9 and dashes)` };
  const name = str(o.name, 60);
  if (!name) return { warning: `${id}: missing name` };
  if (typeof o.type !== 'string' || !WIDGET_TYPES.includes(o.type as WidgetType)) return { warning: `${id}: unknown widget type "${String(o.type)}"` };

  const entry: CatalogEntry = {
    id, name,
    description: str(o.description, 300),
    author: str(o.author, 60) || 'Unknown',
    icon: Array.from(str(o.icon, 16)).slice(0, 4).join('') || '🧩',
    category: str(o.category, 30) || 'Productivity',
    version: str(o.version, 20) || '1.0.0',
    type: o.type as WidgetType,
  };
  if (o.size && typeof o.size === 'object') {
    const w = num((o.size as Record<string, unknown>).w, 140, 800), h = num((o.size as Record<string, unknown>).h, 90, 800);
    if (w && h) entry.size = { w, h };
  }
  if (o.config && typeof o.config === 'object' && !Array.isArray(o.config)) {
    const cfg: Record<string, ConfigValue> = {};
    for (const [k, v] of Object.entries(o.config as Record<string, unknown>).slice(0, MAX_CONFIG_KEYS)) {
      if (!/^[A-Za-z][A-Za-z0-9_]{0,31}$/.test(k)) continue;
      if (typeof v === 'string') cfg[k] = v.slice(0, MAX_STRING);
      else if (typeof v === 'number' && Number.isFinite(v)) cfg[k] = v;
      else if (typeof v === 'boolean') cfg[k] = v;
    }
    if (Object.keys(cfg).length) entry.config = cfg;
  }
  if (typeof o.accent === 'string' && /^#[0-9a-fA-F]{6}$/.test(o.accent)) entry.accent = o.accent.toLowerCase();
  if (Array.isArray(o.tags)) entry.tags = o.tags.filter((t): t is string => typeof t === 'string').map((t) => t.trim().slice(0, 24)).filter(Boolean).slice(0, 6);
  if (o.builtin === true && (BUILTIN_WIDGET_TYPES as string[]).includes(entry.type)) entry.builtin = true;
  return { entry };
}

/** Accepts the whole document (`{ widgets: [...] }`) or a bare array; skips bad entries and duplicates. */
export function sanitizeCatalog(raw: unknown): CatalogResult {
  const list = Array.isArray(raw) ? raw : (raw as { widgets?: unknown } | null)?.widgets;
  if (!Array.isArray(list)) return { entries: [], warnings: ['the catalogue has no "widgets" list'] };
  const entries: CatalogEntry[] = [];
  const warnings: string[] = [];
  const seen = new Set<string>();
  for (const item of list.slice(0, 500)) {
    const { entry, warning } = sanitizeEntry(item);
    if (warning) { warnings.push(warning); continue; }
    if (!entry || seen.has(entry.id)) { if (entry) warnings.push(`${entry.id}: duplicate id`); continue; }
    seen.add(entry.id);
    entries.push(entry);
  }
  return { entries, warnings };
}

/** The catalogue bundled with this build — used offline and when the online one cannot be read. */
export function bundledCatalog(): CatalogResult {
  return sanitizeCatalog(widgetsStoreFallback);
}

/** Entries that are not built in, i.e. the ones that can be installed / removed. */
export function installable(entries: CatalogEntry[]): CatalogEntry[] {
  return entries.filter((e) => !e.builtin);
}

export function searchCatalog(entries: CatalogEntry[], query: string, category = 'all'): CatalogEntry[] {
  const q = query.trim().toLowerCase();
  return entries.filter((e) => {
    if (category !== 'all' && e.category !== category) return false;
    if (!q) return true;
    return [e.name, e.description, e.author, e.category, ...(e.tags ?? [])].some((s) => s.toLowerCase().includes(q));
  });
}

export function isStoreType(t: WidgetType): boolean {
  return (STORE_WIDGET_TYPES as string[]).includes(t);
}
