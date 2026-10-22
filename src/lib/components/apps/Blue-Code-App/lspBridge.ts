import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { snippet, type Completion, type CompletionContext, type CompletionResult, type CompletionSource } from '@codemirror/autocomplete';
import { hoverTooltip, keymap, EditorView } from '@codemirror/view';
import { Prec, type EditorState, type Extension } from '@codemirror/state';
import type { Diagnostic } from './types';
import type { EditorHandle } from './editorHandle';
import { lspSnippetToCm } from './snippetSyntax';
export { lspSnippetToCm };

/**
 * Languages served by a real LSP server through the Rust backend.
 * With Monaco, TypeScript/JavaScript came with a built-in language service;
 * CodeMirror has none, so they now go through `typescript-language-server`
 * like the rest.
 */
export const BRIDGED_LANGS = ['rust', 'python', 'go', 'c', 'cpp', 'typescript', 'javascript'];

export interface LspDiagnosticOut {
  message: string; severity: number; line: number; character: number;
  endLine: number; endCharacter: number; source?: string | null; code?: string | null;
}

/** LSP `CompletionItemKind` → CodeMirror completion `type` (drives the icon). */
export function lspKindToCompletionType(kind?: number | null): string {
  const map: Record<number, string> = {
    1: 'text', 2: 'method', 3: 'function', 4: 'function', 5: 'property', 6: 'variable', 7: 'class', 8: 'interface',
    9: 'namespace', 10: 'property', 11: 'constant', 12: 'constant', 13: 'enum', 14: 'keyword', 15: 'text', 16: 'constant',
    17: 'text', 18: 'text', 19: 'text', 20: 'enum', 21: 'constant', 22: 'class', 23: 'text',
    24: 'keyword', 25: 'type',
  };
  return (kind && map[kind]) || 'text';
}

/** Diagnostyki LSP (0-based) → model Problems panelu Blue Code (1-based). */
export function toProblems(path: string, list: LspDiagnosticOut[]): Diagnostic[] {
  return list.map((d) => ({
    file: path, line: d.line + 1, col: d.character + 1,
    severity: d.severity <= 1 ? 'error' : 'warning',
    message: d.source ? `${d.message} (${d.source}${d.code ? `: ${d.code}` : ''})` : d.message,
  }));
}

export interface LspBridgeDeps {
  getEditor: () => EditorHandle | null;
  getRoot: () => string;
  getActivePath: () => string | undefined;
  onDiagnostics: (path: string, problems: Diagnostic[]) => void;
  openLocation: (path: string, line: number) => void;
}

interface LspLocation { path: string; line: number; character: number; endLine: number; endCharacter: number }

export function createLspBridge(deps: LspBridgeDeps) {
  const ready = new Set<string>();            // języki po udanym handshake
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  let diagListening = false;

  const isActive = (language: string) => ready.has(language);

  async function listenDiagnostics() {
    if (diagListening) return;
    diagListening = true;
    try {
      await listen<{ key: string; path: string; diagnostics: LspDiagnosticOut[] }>('lsp-diagnostics', (e) => {
        const { path, diagnostics } = e.payload;
        deps.onDiagnostics(path, toProblems(path, diagnostics));
        const editor = deps.getEditor();
        if (editor && deps.getActivePath() === path) {
          editor.setMarkers('blue-lsp', diagnostics.map((d) => ({
            severity: d.severity <= 1 ? 'error' as const : d.severity === 2 ? 'warning' as const : 'info' as const,
            startLine: d.line + 1, startCol: d.character + 1,
            endLine: d.endLine + 1, endCol: Math.max(d.endCharacter + 1, d.character + 2),
            message: d.message, source: d.source ?? undefined,
          })));
        }
      });
    } catch { /* poza Tauri */ }
  }

  const argsAt = (language: string, state: EditorState, pos: number) => {
    const line = state.doc.lineAt(pos);
    return { language, rootPath: deps.getRoot(), path: deps.getActivePath(), line: line.number - 1, character: pos - line.from };
  };

  /** Completion source backed by `lsp_completion`. */
  function completionSource(language: string): CompletionSource {
    return async (ctx: CompletionContext): Promise<CompletionResult | null> => {
      if (!isActive(language) || !deps.getActivePath()) return null;
      const word = ctx.matchBefore(/[\w$]*/);
      if (!ctx.explicit && (!word || (word.from === word.to && !/[.:>/<"(]/.test(ctx.state.sliceDoc(Math.max(0, ctx.pos - 1), ctx.pos))))) return null;
      try {
        const items = await invoke<{ label: string; kind?: number; detail?: string; documentation?: string; insertText: string; snippet: boolean }[]>(
          'lsp_completion', argsAt(language, ctx.state, ctx.pos));
        const options: Completion[] = items.map((i) => ({
          label: i.label,
          type: lspKindToCompletionType(i.kind),
          detail: i.detail ?? undefined,
          info: i.documentation ?? undefined,
          apply: i.snippet ? snippet(lspSnippetToCm(i.insertText)) : (i.insertText === i.label ? undefined : i.insertText),
        }));
        return { from: word ? word.from : ctx.pos, options, validFor: /^[\w$]*$/ };
      } catch { return null; }
    };
  }

  /** Hover (type info / docs) from the server. */
  function hover(language: string): Extension {
    return hoverTooltip(async (view, pos) => {
      if (!isActive(language) || !deps.getActivePath()) return null;
      try {
        const h = await invoke<{ contents: string } | null>('lsp_hover', argsAt(language, view.state, pos));
        if (!h || !h.contents) return null;
        const text = h.contents.replace(/```[a-z]*\n?/gi, '').trim();
        return {
          pos, above: true,
          create() {
            const dom = document.createElement('pre');
            dom.style.cssText = 'margin:0;padding:6px 10px;font-size:12px;max-width:520px;max-height:260px;overflow:auto;white-space:pre-wrap';
            dom.textContent = text;
            return { dom };
          },
        };
      } catch { return null; }
    }, { hoverTime: 350 });
  }

  /** Go to definition: Ctrl/Cmd+click and F12. Targets in other files open in Blue Code. */
  function definition(language: string): Extension {
    const go = async (view: EditorView, pos: number) => {
      if (!isActive(language) || !deps.getActivePath()) return false;
      try {
        const locs = await invoke<LspLocation[]>('lsp_definition', argsAt(language, view.state, pos));
        const l = locs?.[0];
        if (!l) return false;
        if (l.path === deps.getActivePath()) {
          const ln = view.state.doc.line(Math.min(l.line + 1, view.state.doc.lines));
          const at = Math.min(ln.from + l.character, ln.to);
          view.dispatch({ selection: { anchor: at }, effects: EditorView.scrollIntoView(at, { y: 'center' }) });
          view.focus();
        } else {
          deps.openLocation(l.path, l.line + 1);
        }
        return true;
      } catch { return false; }
    };
    return [
      EditorView.domEventHandlers({
        mousedown(e, view) {
          if (!(e.ctrlKey || e.metaKey) || e.button !== 0) return false;
          const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
          if (pos == null) return false;
          e.preventDefault();
          void go(view, pos);
          return true;
        },
      }),
      Prec.high(keymap.of([{ key: 'F12', run: (view) => { void go(view, view.state.selection.main.head); return true; } }])),
    ];
  }

  /** Handshake. Returns true when LSP works for this language. */
  async function initialize(language: string): Promise<boolean> {
    if (!BRIDGED_LANGS.includes(language)) return false;
    if (ready.has(language)) return true;
    try {
      await invoke('lsp_initialize', { language, rootPath: deps.getRoot() });
      ready.add(language);
      await listenDiagnostics();
      return true;
    } catch { return false; }
  }

  const call = (cmd: string, a: Record<string, unknown>) => invoke(cmd, a).catch(() => {});

  function didOpen(language: string, path: string, text: string) {
    if (!isActive(language)) return;
    call('lsp_did_open', { language, rootPath: deps.getRoot(), path, languageId: language, text });
  }
  /** Debounce 250 ms — serwer dostaje pełną treść dopiero, gdy użytkownik zrobi pauzę. */
  function didChange(language: string, path: string, text: string) {
    if (!isActive(language)) return;
    clearTimeout(timers.get(path));
    timers.set(path, setTimeout(() => call('lsp_did_change', { language, rootPath: deps.getRoot(), path, text }), 250));
  }
  function didClose(language: string, path: string) {
    if (!isActive(language)) return;
    clearTimeout(timers.get(path));
    call('lsp_did_close', { language, rootPath: deps.getRoot(), path });
  }

  return { initialize, isActive, didOpen, didChange, didClose, completionSource, hover, definition };
}

export type LspBridge = ReturnType<typeof createLspBridge>;
