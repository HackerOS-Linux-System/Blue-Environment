import { Scissors, Copy, ClipboardPaste, TextSelect, Undo2, Redo2, Minus, Maximize2, X as XIcon } from 'lucide-svelte';
import { showContextMenu, contextMenuSerial, type MenuItem } from '../stores/contextMenu';

/**
 * Shell-wide right-click policy.
 *
 * The webview's NATIVE context menu ("Back / Forward / Reload / Inspect
 * element") must never appear in a desktop shell. This installs one window
 * listener that runs AFTER every component's own handler (bubble phase):
 *
 *   - a component that handled the click itself (called preventDefault —
 *     Desktop, Explorer, Monaco…) is left alone;
 *   - otherwise the native menu is suppressed and, where there's something
 *     meaningful to offer, a shell menu is shown instead:
 *       · text fields  → Undo / Cut / Copy / Paste / Select all
 *       · selected text → Copy
 *       · window title bars → Minimize / Maximize / Close (via `data-window-id`)
 *     Plain areas with no actions are deliberately silent rather than showing
 *     an empty menu.
 */
function isEditable(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  if (!el || !el.closest) return false;
  const f = el.closest('input, textarea, [contenteditable=""], [contenteditable="true"]') as HTMLElement | null;
  if (!f) return false;
  if (f instanceof HTMLInputElement) return !['checkbox', 'radio', 'button', 'range', 'color', 'file', 'submit'].includes(f.type);
  return true;
}

function editableMenu(el: HTMLElement): MenuItem[] {
  const input = el as HTMLInputElement | HTMLTextAreaElement;
  const isField = 'selectionStart' in input && typeof input.selectionStart === 'number';
  const selected = isField ? input.selectionStart !== input.selectionEnd : !!window.getSelection()?.toString();
  const readonly = (input as any).readOnly || (input as any).disabled;
  const focus = () => el.focus();

  return [
    { label: 'Undo', icon: Undo2, shortcut: 'Ctrl+Z', disabled: readonly, action: () => { focus(); document.execCommand('undo'); } },
    { label: 'Redo', icon: Redo2, shortcut: 'Ctrl+Shift+Z', disabled: readonly, action: () => { focus(); document.execCommand('redo'); } },
    { separator: true },
    { label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X', disabled: !selected || readonly, action: () => { focus(); document.execCommand('cut'); } },
    { label: 'Copy', icon: Copy, shortcut: 'Ctrl+C', disabled: !selected, action: () => { focus(); document.execCommand('copy'); } },
    {
      label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+V', disabled: readonly,
      action: async () => {
        focus();
        let text = '';
        try { text = await navigator.clipboard.readText(); } catch { return; }
        if (!text) return;
        // insertText keeps the field's undo stack and fires `input` so Svelte bindings update.
        if (!document.execCommand('insertText', false, text) && isField) {
          input.setRangeText(text, input.selectionStart ?? 0, input.selectionEnd ?? 0, 'end');
          input.dispatchEvent(new Event('input', { bubbles: true }));
        }
      },
    },
    { separator: true },
    { label: 'Select all', icon: TextSelect, shortcut: 'Ctrl+A', action: () => { focus(); document.execCommand('selectAll'); } },
  ];
}

export function installGlobalContextMenu(windowActions?: {
  minimize: (id: string) => void; maximize: (id: string) => void; close: (id: string) => void;
}): () => void {
  let before = 0;
  const onCapture = () => { before = contextMenuSerial(); };

  const onBubble = (e: MouseEvent) => {
    // Always: no native webview menu, whatever happens below.
    const alreadyHandled = e.defaultPrevented || contextMenuSerial() !== before;
    e.preventDefault();
    if (alreadyHandled) return;

    const target = e.target as HTMLElement | null;
    if (!target?.closest) return;

    // Monaco draws its own context menu.
    if (target.closest('.monaco-editor')) return;

    if (isEditable(target)) {
      const field = target.closest('input, textarea, [contenteditable]') as HTMLElement;
      showContextMenu(e, editableMenu(field));
      return;
    }

    const sel = window.getSelection()?.toString();
    if (sel) {
      showContextMenu(e, [
        { label: 'Copy', icon: Copy, shortcut: 'Ctrl+C', action: () => { document.execCommand('copy'); } },
        { label: 'Select all', icon: TextSelect, shortcut: 'Ctrl+A', action: () => { document.execCommand('selectAll'); } },
      ]);
      return;
    }

    const titleBar = target.closest('[data-window-id]') as HTMLElement | null;
    if (titleBar && windowActions && target.closest('[data-window-titlebar]')) {
      const id = titleBar.dataset.windowId!;
      showContextMenu(e, [
        { label: 'Minimize', icon: Minus, action: () => windowActions.minimize(id) },
        { label: 'Maximize / Restore', icon: Maximize2, action: () => windowActions.maximize(id) },
        { separator: true },
        { label: 'Close', icon: XIcon, danger: true, action: () => windowActions.close(id) },
      ]);
    }
  };

  window.addEventListener('contextmenu', onCapture, true);
  window.addEventListener('contextmenu', onBubble, false);
  return () => {
    window.removeEventListener('contextmenu', onCapture, true);
    window.removeEventListener('contextmenu', onBubble, false);
  };
}
