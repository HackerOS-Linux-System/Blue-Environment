import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';

export interface Rect { x: number; y: number; width: number; height: number }
export interface Reserved { top: number; right: number; bottom: number; left: number }
export type Corner = 'tl' | 'tr' | 'bl' | 'br';

/** Wysokość paska tytułu / paska sterowania ramki oraz obramowanie (px). */
export const PIP_TITLE_H = 28;
export const PIP_CTRL_H = 38;
export const PIP_BORDER = 1;
export const PIP_MIN_W = 240;
export const PIP_MAX_W = 720;
export const PIP_DEFAULT_W = 380;
export const PIP_MARGIN = 16;

export interface PipSession {
  /** Okno Blue Web, z którego pochodzi karta. */
  windowId: string;
  tabId: string;
  title: string;
  url: string;
  favicon?: string;
  /** Lewy górny róg całej ramki (z paskami), w pikselach powłoki. */
  x: number;
  y: number;
  /** Szerokość całej ramki. Wysokość wynika z proporcji wideo. */
  width: number;
  /** Proporcje wideo (szer./wys.). */
  aspect: number;
  corner: Corner;
}

export interface VideoInfo {
  hasVideo: boolean;
  playing: boolean;
  muted: boolean;
  aspect: number;
}

/** Aktywna sesja PiP (tylko jedna naraz, jak w Androidzie). */
export const webPip = writable<PipSession | null>(null);
/** Ostatni znany stan wideo każdej śledzonej karty. */
export const videoInfo = writable<Record<string, VideoInfo>>({});

// ── czysta geometria (testowana w pip.test.ts) ─────────────────────────

export function clampAspect(a: number): number {
  if (!Number.isFinite(a)) return 16 / 9;
  return Math.min(3, Math.max(0.4, a));
}

/** Szerokość miejsca na wideo (bez obramowania). */
export const innerWidth = (frameWidth: number) => Math.max(1, frameWidth - 2 * PIP_BORDER);

export function videoHeight(frameWidth: number, aspect: number): number {
  return Math.max(1, Math.round(innerWidth(frameWidth) / clampAspect(aspect)));
}

/** Pełna wysokość ramki: tytuł + wideo + sterowanie + obramowanie. */
export function frameHeight(frameWidth: number, aspect: number): number {
  return PIP_TITLE_H + videoHeight(frameWidth, aspect) + PIP_CTRL_H + 2 * PIP_BORDER;
}

/** Prostokąt, w którym ma stać natywny webview (współrzędne powłoki). */
export function videoRect(s: Pick<PipSession, 'x' | 'y' | 'width' | 'aspect'>): Rect {
  return {
    x: s.x + PIP_BORDER,
    y: s.y + PIP_BORDER + PIP_TITLE_H,
    width: innerWidth(s.width),
    height: videoHeight(s.width, s.aspect),
  };
}

export function frameRect(s: Pick<PipSession, 'x' | 'y' | 'width' | 'aspect'>): Rect {
  return { x: s.x, y: s.y, width: s.width, height: frameHeight(s.width, s.aspect) };
}

/** Szerokość mieści się w limitach i w ekranie (max 60% szer. i 70% wys.). */
export function clampWidth(width: number, aspect: number, vw: number, vh: number, r: Reserved): number {
  const availW = Math.max(PIP_MIN_W, vw - r.left - r.right - 2 * PIP_MARGIN);
  const availH = Math.max(1, vh - r.top - r.bottom - 2 * PIP_MARGIN);
  // Najwyższa szerokość, przy której ramka jeszcze mieści się w pionie.
  const chrome = PIP_TITLE_H + PIP_CTRL_H + 2 * PIP_BORDER;
  const byHeight = Math.floor((availH - chrome) * clampAspect(aspect)) + 2 * PIP_BORDER;
  const max = Math.max(PIP_MIN_W, Math.min(PIP_MAX_W, Math.floor(vw * 0.6), availW, byHeight));
  return Math.round(Math.min(max, Math.max(PIP_MIN_W, width)));
}

/** Utrzymuje całą ramkę na ekranie (poza zarezerwowanymi paskami). */
export function clampPosition(s: Pick<PipSession, 'x' | 'y' | 'width' | 'aspect'>, vw: number, vh: number, r: Reserved): { x: number; y: number } {
  const h = frameHeight(s.width, s.aspect);
  const minX = r.left;
  const maxX = Math.max(minX, vw - r.right - s.width);
  const minY = r.top;
  const maxY = Math.max(minY, vh - r.bottom - h);
  return { x: Math.round(Math.min(maxX, Math.max(minX, s.x))), y: Math.round(Math.min(maxY, Math.max(minY, s.y))) };
}

export function cornerPosition(corner: Corner, width: number, aspect: number, vw: number, vh: number, r: Reserved): { x: number; y: number } {
  const h = frameHeight(width, aspect);
  const left = r.left + PIP_MARGIN;
  const right = vw - r.right - PIP_MARGIN - width;
  const top = r.top + PIP_MARGIN;
  const bottom = vh - r.bottom - PIP_MARGIN - h;
  const x = corner === 'tl' || corner === 'bl' ? left : right;
  const y = corner === 'tl' || corner === 'tr' ? top : bottom;
  return clampPosition({ x, y, width, aspect }, vw, vh, r);
}

/** Najbliższy róg ekranu dla środka ramki (przyciąganie po puszczeniu). */
export function nearestCorner(s: Pick<PipSession, 'x' | 'y' | 'width' | 'aspect'>, vw: number, vh: number, r: Reserved): Corner {
  const cx = s.x + s.width / 2;
  const cy = s.y + frameHeight(s.width, s.aspect) / 2;
  const midX = (r.left + (vw - r.right)) / 2;
  const midY = (r.top + (vh - r.bottom)) / 2;
  return `${cy < midY ? 't' : 'b'}${cx < midX ? 'l' : 'r'}` as Corner;
}

// ── zapamiętane ustawienia (rozmiar i róg) ─────────────────────────────

const PREFS_KEY = 'blue-web-pip-prefs';
export interface PipPrefs { width: number; corner: Corner }
const CORNERS: Corner[] = ['tl', 'tr', 'bl', 'br'];

export function loadPipPrefs(): PipPrefs {
  try {
    const raw = JSON.parse(localStorage.getItem(PREFS_KEY) ?? 'null');
    const width = Number(raw?.width);
    return {
      width: Number.isFinite(width) ? Math.min(PIP_MAX_W, Math.max(PIP_MIN_W, Math.round(width))) : PIP_DEFAULT_W,
      corner: CORNERS.includes(raw?.corner) ? raw.corner : 'br',
    };
  } catch {
    return { width: PIP_DEFAULT_W, corner: 'br' };
  }
}

export function savePipPrefs(p: PipPrefs) {
  try { localStorage.setItem(PREFS_KEY, JSON.stringify(p)); } catch { /* best effort */ }
}

// ── śledzenie stanu wideo kart (zdarzenia `web-pip-{tabId}`) ───────────

const videoUnlisten = new Map<string, () => void>();

export async function trackVideo(tabId: string) {
  if (!SystemBridge.isTauri() || videoUnlisten.has(tabId)) return;
  videoUnlisten.set(tabId, () => {}); // zajmij miejsce, żeby równoległe wywołania nie dublowały nasłuchu
  try {
    const mod = await import('@tauri-apps/api/event');
    const un = await mod.listen(`web-pip-${tabId}`, (e: any) => {
      const p = e.payload ?? {};
      videoInfo.update((prev) => ({
        ...prev,
        [tabId]: {
          hasVideo: !!p.has_video,
          playing: !!p.playing,
          muted: !!p.muted,
          aspect: clampAspect(Number(p.aspect)),
        },
      }));
    });
    if (videoUnlisten.has(tabId)) videoUnlisten.set(tabId, un); else un();
  } catch {
    videoUnlisten.delete(tabId);
  }
}

export function untrackVideo(tabId: string) {
  videoUnlisten.get(tabId)?.();
  videoUnlisten.delete(tabId);
  videoInfo.update((prev) => {
    if (!(tabId in prev)) return prev;
    const { [tabId]: _gone, ...rest } = prev;
    return rest;
  });
}

// ── polecenia ──────────────────────────────────────────────────────────

export const isPipTab = (tabId: string) => get(webPip)?.tabId === tabId;

export interface EnterPipArgs {
  windowId: string;
  tabId: string;
  title: string;
  url: string;
  favicon?: string;
}

/** Paski powłoki (panel), których ramka PiP nie może zasłaniać. Ustawia je WebPipOverlay. */
export const pipReserved = writable<Reserved>({ top: 0, right: 0, bottom: 0, left: 0 });

/**
 * Wchodzi w PiP z kartą `tabId`. Stan (`webPip`) ustawiany jest NATYCHMIAST,
 * zanim cokolwiek asynchronicznego się wydarzy — dzięki temu okno Blue Web od
 * razu przestaje ruszać natywnym webview tej karty. Zwraca `false` poza Tauri.
 */
export async function enterPip(a: EnterPipArgs): Promise<boolean> {
  if (!SystemBridge.isTauri()) return false;
  const current = get(webPip);
  if (current && current.tabId === a.tabId) return true;
  if (current) await exitPip();

  const prefs = loadPipPrefs();
  const vw = window.innerWidth, vh = window.innerHeight, reserved = get(pipReserved);
  const aspect = clampAspect(get(videoInfo)[a.tabId]?.aspect ?? 16 / 9);
  const width = clampWidth(prefs.width, aspect, vw, vh, reserved);
  const pos = cornerPosition(prefs.corner, width, aspect, vw, vh, reserved);
  webPip.set({
    windowId: a.windowId, tabId: a.tabId, title: a.title, url: a.url, favicon: a.favicon,
    ...pos, width, aspect, corner: prefs.corner,
  });
  try {
    await SystemBridge.invokeCommand('web_view_set_clip', { tabId: a.tabId }); // zdejmij ewentualne przycięcie okna
  } catch { /* clip-unsupported to normalna odpowiedź */ }
  try {
    await SystemBridge.invokeCommand('web_pip_enter', { tabId: a.tabId });
  } catch {
    webPip.set(null);
    return false;
  }
  return true;
}

/** Kończy PiP: strona wraca do normalnego wyglądu, natywny widok jest chowany
 * (okno Blue Web pokaże go ponownie, jeśli karta jest u niego aktywna). */
export async function exitPip(): Promise<void> {
  const s = get(webPip);
  if (!s) return;
  if (SystemBridge.isTauri()) {
    try { await SystemBridge.invokeCommand('web_pip_exit', { tabId: s.tabId }); } catch { /* karta mogła zniknąć */ }
    try { await SystemBridge.invokeCommand('web_view_set_interactive', { tabId: s.tabId, interactive: true }); } catch { /* ignore */ }
    try { await SystemBridge.invokeCommand('web_view_set_visible', { tabId: s.tabId, visible: false }); } catch { /* ignore */ }
  }
  if (get(webPip)?.tabId === s.tabId) webPip.set(null);
}

/**
 * Zamknięcie PiP przyciskiem ×. Jeśli okno Blue Web, do którego karta należy,
 * jeszcze żyje — karta po prostu wraca do niego (ukryta, jak każda nieaktywna).
 * Jeśli okna już nie ma (zminimalizowane / inny pulpit) — karta jest sierotą:
 * zamykamy jej webview. Zwraca id sieroty (do posprzątania w tabs.ts) lub `null`.
 */
export async function closePip(): Promise<string | null> {
  const s = get(webPip);
  if (!s) return null;
  const orphan = !hosts.has(s.windowId);
  await exitPip();
  if (!orphan) return null;
  pendingAdopt = null;
  if (SystemBridge.isTauri()) {
    try { await SystemBridge.invokeCommand('web_view_close', { tabId: s.tabId }); } catch { /* już zamknięta */ }
  }
  untrackVideo(s.tabId);
  return s.tabId;
}

/** Zamyka PiP razem z kartą (karta zamknięta / okno zamknięte). */
export function dropPip(tabId: string) {
  if (get(webPip)?.tabId === tabId) webPip.set(null);
  untrackVideo(tabId);
}

export async function controlPip(action: 'play' | 'pause' | 'toggle' | 'mute' | 'unmute' | 'toggle_mute' | 'seek', value?: number) {
  const s = get(webPip);
  if (!s || !SystemBridge.isTauri()) return;
  try { await SystemBridge.invokeCommand('web_pip_control', { tabId: s.tabId, action, value }); } catch { /* strona mogła się zmienić */ }
}

// ── rejestr „gospodarzy” (okna Blue Web) i przekazanie karty ──────────

export interface PipHost {
  /** Próbuje włączyć PiP dla aktywnej karty; `true` tylko gdy karta ma znane wideo. */
  requestPip: () => boolean;
  /** Robi kartę aktywną w oknie (przy „wróć do karty”). */
  activateTab: (tabId: string) => void;
}

const hosts = new Map<string, PipHost>();
export const registerPipHost = (windowId: string, h: PipHost) => { hosts.set(windowId, h); };
export const unregisterPipHost = (windowId: string) => { hosts.delete(windowId); };
export const getPipHost = (windowId: string) => hosts.get(windowId);

/** Dla windowManager.togglePiP: czy okno Blue Web obsłużyło żądanie jako PiP wideo. */
export function requestVideoPip(windowId: string): boolean {
  return hosts.get(windowId)?.requestPip() ?? false;
}

/**
 * Okno Blue Web zostało zniszczone (zminimalizowane / zmiana pulpitu), ale PiP
 * żyje dalej. Przy „wróć do karty” nowa instancja okna przejmuje kartę stąd.
 */
export interface PendingAdopt { tabId: string; url: string; title: string; favicon?: string; zoom?: number }
let pendingAdopt: PendingAdopt | null = null;
export const setPendingAdopt = (a: PendingAdopt | null) => { pendingAdopt = a; };
export function takePendingAdopt(): PendingAdopt | null {
  const a = pendingAdopt;
  pendingAdopt = null;
  return a;
}
