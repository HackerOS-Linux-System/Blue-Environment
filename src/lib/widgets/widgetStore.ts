import { writable, get } from 'svelte/store';
import { SystemBridge } from '../utils/systemBridge';
import {
  parseWidgets, serializeWidgets, createWidget, updateWidget, bringToFront,
  type WidgetInstance, type WidgetType,
} from './widgetModel';

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

export function addWidget(type: WidgetType) {
  widgets.update((l) => [...l, createWidget(type, l, window.innerWidth, window.innerHeight)]);
  persist();
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
