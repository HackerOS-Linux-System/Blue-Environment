export type ClipboardAction = 'copy' | 'copy-clear' | 'paste' | null;

export interface KeyLike {
  type: string; key: string; code?: string;
  ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean;
}

const is = (e: KeyLike, letter: string) => e.code === `Key${letter.toUpperCase()}` || e.key.toLowerCase() === letter;

export function classifyKey(e: KeyLike, hasSelection: boolean): ClipboardAction {
  if (e.type !== 'keydown' || e.altKey || e.metaKey) return null;
  if (e.ctrlKey && e.shiftKey) {
    if (is(e, 'c')) return 'copy';
    if (is(e, 'v')) return 'paste';
    return null;
  }
  if (e.ctrlKey && !e.shiftKey && e.key === 'Insert') return 'copy';
  if (e.shiftKey && !e.ctrlKey && e.key === 'Insert') return 'paste';
  if (e.ctrlKey && !e.shiftKey && is(e, 'c') && hasSelection) return 'copy-clear';
  return null;
}

/** Normalises pasted text for a terminal: CRLF/CR → LF is left to xterm; we only strip a lone trailing NUL. */
export function sanitizePaste(text: string): string {
  return text.replace(/\0/g, '');
}
