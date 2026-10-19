export type RemoteInput =
  | { type: 'key'; name: string }
  | { type: 'text'; text: string }
  | { type: 'move'; dx: number; dy: number }
  | { type: 'click'; button: 'left' | 'right' | 'middle' | string }
  | { type: 'scroll'; dy: number }
  | { type: 'volume'; delta: number }
  | { type: 'mute' }
  | { type: 'media'; action: string };

export interface RemoteEventPayload { input: RemoteInput; handledBySystem: boolean }

export type NavAction = 'up' | 'down' | 'left' | 'right' | 'select' | 'back' | 'home' | 'menu';

const KEY_TO_ACTION: Record<string, NavAction> = {
  up: 'up', down: 'down', left: 'left', right: 'right', select: 'select', enter: 'select', back: 'back', home: 'home', menu: 'menu',
};

export const keyToAction = (name: string): NavAction | null => KEY_TO_ACTION[name] ?? null;

export const MEDIA_ACTIONS = ['play_pause', 'next', 'prev', 'stop'] as const;

/** Inserts `text` at the selection of an input value. */
export function insertText(value: string, start: number, end: number, text: string): { value: string; caret: number } {
  const s = Math.max(0, Math.min(start, value.length));
  const e = Math.max(s, Math.min(end, value.length));
  return { value: value.slice(0, s) + text + value.slice(e), caret: s + text.length };
}

/** Backspace: deletes the selection, or the character before the caret. */
export function backspace(value: string, start: number, end: number): { value: string; caret: number } {
  const s = Math.max(0, Math.min(start, value.length));
  const e = Math.max(s, Math.min(end, value.length));
  if (e > s) return { value: value.slice(0, s) + value.slice(e), caret: s };
  if (s === 0) return { value, caret: 0 };
  return { value: value.slice(0, s - 1) + value.slice(s), caret: s - 1 };
}

export interface Cursor { x: number; y: number }

/** Moves the virtual cursor, keeping it on screen. */
export function moveCursor(c: Cursor, dx: number, dy: number, w: number, h: number): Cursor {
  return { x: Math.max(0, Math.min(w - 1, c.x + dx)), y: Math.max(0, Math.min(h - 1, c.y + dy)) };
}

const EDITABLE_INPUT_TYPES = new Set(['text', 'search', 'url', 'tel', 'email', 'password', 'number']);

export function isTextEditable(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement | HTMLElement {
  if (!el) return false;
  if (el.tagName === 'TEXTAREA') return true;
  if (el.tagName === 'INPUT') return EDITABLE_INPUT_TYPES.has(((el as HTMLInputElement).type || 'text').toLowerCase());
  return (el as HTMLElement).isContentEditable === true;
}

/** Writes into a focused field and notifies frameworks (`input` event). Returns whether anything was edited. */
export function editFocused(doc: Document, op: { kind: 'text'; text: string } | { kind: 'backspace' }): boolean {
  const el = doc.activeElement;
  if (!isTextEditable(el)) return false;
  if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
    const f = el as HTMLInputElement;
    // `number` inputs have no selection API — fall back to appending.
    let start = f.value.length; let end = start;
    try { start = f.selectionStart ?? f.value.length; end = f.selectionEnd ?? start; } catch { /* number input */ }
    const r = op.kind === 'text' ? insertText(f.value, start, end, op.text) : backspace(f.value, start, end);
    f.value = r.value;
    try { f.setSelectionRange(r.caret, r.caret); } catch { /* number input */ }
    f.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  }
  // contenteditable
  if (op.kind === 'text') doc.execCommand?.('insertText', false, op.text);
  else doc.execCommand?.('delete');
  return true;
}

const FOCUSABLE = 'a[href],button:not([disabled]),input:not([disabled]):not([type=hidden]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';

/** Desktop fallback for D-pad navigation: arrow keys go to the focused element, OK clicks it. */
export function fallbackNavigate(doc: Document, action: NavAction): void {
  const target: Element = doc.activeElement && doc.activeElement !== doc.body ? doc.activeElement : doc.body;
  const key: Record<NavAction, string | null> = { up: 'ArrowUp', down: 'ArrowDown', left: 'ArrowLeft', right: 'ArrowRight', back: 'Escape', select: null, home: null, menu: 'ContextMenu' };
  if (action === 'select') { (target as HTMLElement).click?.(); return; }
  const k = key[action];
  if (k) target.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
}

/** Moves focus to the next/previous focusable element (Tab / Shift+Tab). */
export function focusStep(doc: Document, dir: 1 | -1): void {
  const els = Array.from(doc.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((e) => e.offsetParent !== null || e === doc.activeElement);
  if (!els.length) return;
  const i = els.indexOf(doc.activeElement as HTMLElement);
  els[(i + dir + els.length) % els.length].focus();
}

// ── virtual cursor ──

let cursorEl: HTMLElement | null = null;
let cursorPos: Cursor | null = null;
let hideTimer: ReturnType<typeof setTimeout> | undefined;

function ensureCursor(doc: Document, win: Window): HTMLElement {
  if (!cursorPos) cursorPos = { x: win.innerWidth / 2, y: win.innerHeight / 2 };
  if (!cursorEl || !doc.body.contains(cursorEl)) {
    cursorEl = doc.createElement('div');
    cursorEl.setAttribute('aria-hidden', 'true');
    cursorEl.style.cssText = 'position:fixed;left:0;top:0;width:28px;height:28px;z-index:2147483647;pointer-events:none;transition:opacity .2s;filter:drop-shadow(0 2px 3px rgba(0,0,0,.6))';
    cursorEl.innerHTML = '<svg viewBox="0 0 24 24" width="28" height="28"><path d="M3 2l7 19 2.6-7.4L20 11z" fill="#fff" stroke="#0b1220" stroke-width="1.4" stroke-linejoin="round"/></svg>';
    doc.body.appendChild(cursorEl);
  }
  return cursorEl;
}

function showCursor(doc: Document, win: Window) {
  const el = ensureCursor(doc, win);
  el.style.opacity = '1';
  el.style.transform = `translate(${cursorPos!.x}px,${cursorPos!.y}px)`;
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => { if (cursorEl) cursorEl.style.opacity = '0'; }, 3500);
}

function scrollableAt(doc: Document, x: number, y: number): Element {
  let el: Element | null = doc.elementFromPoint(x, y);
  while (el && el !== doc.body) {
    const cs = getComputedStyle(el);
    if ((/(auto|scroll)/.test(cs.overflowY)) && el.scrollHeight > el.clientHeight) return el;
    el = el.parentElement;
  }
  return doc.scrollingElement ?? doc.body;
}

export interface Env {
  doc: Document;
  win: Window;
  /** Called with `blue:remote-action`; a listener sets `handled` when it consumed the action. */
  dispatch: (name: string, detail: unknown) => void;
}

export function defaultEnv(): Env {
  return { doc: document, win: window, dispatch: (name, detail) => window.dispatchEvent(new CustomEvent(name, { detail })) };
}

/** Applies one validated input from the backend. */
export function handleRemoteInput(p: RemoteEventPayload, env: Env = defaultEnv()): void {
  const { input } = p;
  const { doc, win } = env;
  switch (input.type) {
    case 'media':
      env.dispatch('blue:media-key', input.action);
      return;
    case 'volume':
    case 'mute':
      return; // done by the backend (wpctl / pactl)
    default:
      break;
  }
  if (p.handledBySystem) return; // ydotool already did it system-wide
  switch (input.type) {
    case 'key': {
      if (input.name === 'backspace') { editFocused(doc, { kind: 'backspace' }); return; }
      if (input.name === 'tab') { focusStep(doc, 1); return; }
      if (input.name === 'space') { if (!editFocused(doc, { kind: 'text', text: ' ' })) keyToActionDispatch('select', env); return; }
      const a = keyToAction(input.name);
      if (a) keyToActionDispatch(a, env);
      return;
    }
    case 'text':
      editFocused(doc, { kind: 'text', text: input.text });
      return;
    case 'move': {
      ensureCursor(doc, win);
      cursorPos = moveCursor(cursorPos!, input.dx, input.dy, win.innerWidth, win.innerHeight);
      showCursor(doc, win);
      return;
    }
    case 'click': {
      ensureCursor(doc, win);
      showCursor(doc, win);
      const el = doc.elementFromPoint(cursorPos!.x, cursorPos!.y) as HTMLElement | null;
      if (!el) return;
      const init = { bubbles: true, cancelable: true, clientX: cursorPos!.x, clientY: cursorPos!.y, button: input.button === 'right' ? 2 : input.button === 'middle' ? 1 : 0 };
      if (input.button === 'right') { el.dispatchEvent(new MouseEvent('contextmenu', init)); return; }
      el.focus?.();
      el.dispatchEvent(new MouseEvent('mousedown', init));
      el.dispatchEvent(new MouseEvent('mouseup', init));
      el.click?.();
      return;
    }
    case 'scroll': {
      ensureCursor(doc, win);
      const t = scrollableAt(doc, cursorPos!.x, cursorPos!.y);
      t.scrollBy?.({ top: input.dy * 60, behavior: 'auto' });
      return;
    }
  }
}

function keyToActionDispatch(a: NavAction, env: Env) {
  const detail = { action: a, handled: false };
  env.dispatch('blue:remote-action', detail);
  if (!detail.handled) fallbackNavigate(env.doc, a);
}

/** Starts listening to the backend. Returns the unlisten function. */
export async function initRemoteReceiver(): Promise<() => void> {
  const { listen } = await import('@tauri-apps/api/event');
  const un = await listen<RemoteEventPayload>('blue-remote://input', (e) => handleRemoteInput(e.payload));
  return () => un();
}

/** Test hook: forget the virtual cursor between tests. */
export function __resetCursorForTests() {
  cursorEl?.remove(); cursorEl = null; cursorPos = null;
  if (hideTimer) clearTimeout(hideTimer);
}
