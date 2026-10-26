import { writable, get } from 'svelte/store';
import { SystemBridge } from '../utils/systemBridge';
import {
  parseWidgets, serializeWidgets, createWidget, updateWidget, bringToFront,
  type WidgetInstance, type WidgetType, type WidgetPreset,
} from './widgetModel';
import { sanitizeCatalog, type CatalogEntry } from './widgetCatalog';

const PATH = 'HOME/.config/Blue-Environment/desktop-widgets.json';

export const widgets = writable<WidgetInstance[]>([]);
/** While true the layer shows drag handles, resize grips, remove buttons and the "add widget" palette. */
export const widgetEditMode = writable(false);

let loaded = false;
let timer: ReturnType<typeof setTimeout> | undefined;

function persist() {
  if (!loaded) return; // never overwrite the file before it was read
  clearTimeout(timer);
  timer = setTimeout(() => {
    SystemBridge.writeFile(PATH, serializeWidgets(get(widgets))).catch(() => {});
  }, 300);
}

export async function loadWidgets(): Promise<void> {
  if (loaded) return;
  try { widgets.set(parseWidgets(await SystemBridge.readFile(PATH))); } catch { widgets.set([]); }
  loaded = true;
}

export function addWidget(type: WidgetType, preset?: WidgetPreset) {
  widgets.update((l) => [...l, createWidget(type, l, window.innerWidth, window.innerHeight, preset)]);
  persist();
}

/** Puts a Widget Store entry on the desktop (size and settings come from the entry). */
export function addCatalogWidget(entry: CatalogEntry) {
  addWidget(entry.type, { storeId: entry.id, size: entry.size, config: entry.config });
}

// ── widgets installed from the Widget Store ──────────────────────────
// Stored as full entries (not just ids) so the desktop palette keeps working
// offline and after an entry changes or disappears from the online catalogue.

const INSTALLED_PATH = 'HOME/.config/Blue-Environment/installed-widgets.json';

export const installedWidgets = writable<CatalogEntry[]>([]);
let installedLoaded = false;

export async function loadInstalledWidgets(): Promise<void> {
  if (installedLoaded) return;
  try {
    const raw = await SystemBridge.readFile(INSTALLED_PATH);
    installedWidgets.set(sanitizeCatalog(JSON.parse(raw)).entries.filter((e) => !e.builtin));
  } catch { installedWidgets.set([]); }
  installedLoaded = true;
}

async function persistInstalled() {
  if (!installedLoaded) return;
  await SystemBridge.writeFile(INSTALLED_PATH, JSON.stringify({ version: 1, widgets: get(installedWidgets) }, null, 2)).catch(() => {});
}

export async function installWidget(entry: CatalogEntry): Promise<void> {
  await loadInstalledWidgets();
  if (entry.builtin) return;
  installedWidgets.update((l) => [...l.filter((e) => e.id !== entry.id), entry]);
  await persistInstalled();
}

/** Removes the entry from the palette and the widgets placed from it from the desktop. */
export async function uninstallWidget(id: string): Promise<void> {
  await loadInstalledWidgets();
  installedWidgets.update((l) => l.filter((e) => e.id !== id));
  widgets.update((l) => l.filter((w) => w.storeId !== id));
  persist();
  await persistInstalled();
}
export function patchWidget(id: string, patch: Partial<WidgetInstance>) {
  widgets.update((l) => updateWidget(l, id, patch));
  persist();
}
export function removeWidget(id: string) {
  widgets.update((l) => l.filter((w) => w.id !== id));
  persist();
}
export function raiseWidget(id: string) {
  widgets.update((l) => bringToFront(l, id));
}
export function savePositions() { persist(); }
