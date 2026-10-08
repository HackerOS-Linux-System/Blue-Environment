import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Diagnostic } from './types';

/** Języki obsługiwane przez serwery LSP, których Monaco nie ma wbudowanych. */
export const BRIDGED_LANGS = ['rust', 'python', 'go', 'c', 'cpp'];

export interface LspDiagnosticOut {
  message: string; severity: number; line: number; character: number;
  endLine: number; endCharacter: number; source?: string | null; code?: string | null;
}

/** Numer `CompletionItemKind` z LSP → nazwa w `monaco.languages.CompletionItemKind`. */
export function lspKindToMonaco(kind?: number | null): string {
  const map: Record<number, string> = {
    1: 'Text', 2: 'Method', 3: 'Function', 4: 'Constructor', 5: 'Field', 6: 'Variable', 7: 'Class', 8: 'Interface',
    9: 'Module', 10: 'Property', 11: 'Unit', 12: 'Value', 13: 'Enum', 14: 'Keyword', 15: 'Snippet', 16: 'Color',
    17: 'File', 18: 'Reference', 19: 'Folder', 20: 'EnumMember', 21: 'Constant', 22: 'Struct', 23: 'Event',
    24: 'Operator', 25: 'TypeParameter',
  };
  return (kind && map[kind]) || 'Text';
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
  getMonaco: () => any;
  getRoot: () => string;
  getActivePath: () => string | undefined;
  onDiagnostics: (path: string, problems: Diagnostic[]) => void;
  openLocation: (path: string, line: number) => void;
}

export function createLspBridge(deps: LspBridgeDeps) {
  const ready = new Set<string>();            // języki po udanym handshake
  const providersFor = new Set<string>();     // języki z zarejestrowanymi providerami Monaco
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
        const monaco = deps.getMonaco();
        if (monaco && deps.getActivePath() === path) {
          const model = monaco.editor.getModels?.()[0];
          if (model) {
            monaco.editor.setModelMarkers(model, 'blue-lsp', diagnostics.map((d) => ({
              severity: d.severity <= 1 ? monaco.MarkerSeverity.Error : d.severity === 2 ? monaco.MarkerSeverity.Warning : monaco.MarkerSeverity.Info,
              startLineNumber: d.line + 1, startColumn: d.character + 1,
              endLineNumber: d.endLine + 1, endColumn: Math.max(d.endCharacter + 1, d.character + 2),
              message: d.message, source: d.source ?? undefined,
            })));
          }
        }
      });
    } catch { /* poza Tauri */ }
  }

  function registerProviders(language: string) {
    const monaco = deps.getMonaco();
    if (!monaco || providersFor.has(language)) return;
    providersFor.add(language);
    const root = () => deps.getRoot();
    const args = (model: any, pos: any) => ({
      language, rootPath: root(), path: deps.getActivePath(), line: pos.lineNumber - 1, character: pos.column - 1,
    });

    monaco.languages.registerHoverProvider(language, {
      provideHover: async (model: any, pos: any) => {
        if (!deps.getActivePath()) return null;
        try {
          const h = await invoke<{ contents: string } | null>('lsp_hover', args(model, pos));
          return h ? { contents: [{ value: h.contents }] } : null;
        } catch { return null; }
      },
    });

    monaco.languages.registerCompletionItemProvider(language, {
      triggerCharacters: ['.', ':', '>', '<', '"', '/', '('],
      provideCompletionItems: async (model: any, pos: any) => {
        if (!deps.getActivePath()) return { suggestions: [] };
        try {
          const items = await invoke<{ label: string; kind?: number; detail?: string; documentation?: string; insertText: string; snippet: boolean }[]>('lsp_completion', args(model, pos));
          const w = model.getWordUntilPosition(pos);
          const range = { startLineNumber: pos.lineNumber, endLineNumber: pos.lineNumber, startColumn: w.startColumn, endColumn: w.endColumn };
          return {
            suggestions: items.map((i) => ({
              label: i.label,
              kind: monaco.languages.CompletionItemKind[lspKindToMonaco(i.kind)],
              detail: i.detail ?? undefined,
              documentation: i.documentation ? { value: i.documentation } : undefined,
              insertText: i.insertText,
              insertTextRules: i.snippet ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet : undefined,
              range,
            })),
          };
        } catch { return { suggestions: [] }; }
      },
    });

    monaco.languages.registerDefinitionProvider(language, {
      provideDefinition: async (model: any, pos: any) => {
        if (!deps.getActivePath()) return null;
        try {
          const locs = await invoke<{ path: string; line: number; character: number; endLine: number; endCharacter: number }[]>('lsp_definition', args(model, pos));
          return locs.map((l) => ({
            uri: monaco.Uri.file(l.path),
            range: { startLineNumber: l.line + 1, startColumn: l.character + 1, endLineNumber: l.endLine + 1, endColumn: l.endCharacter + 1 },
          }));
        } catch { return null; }
      },
    });

    // Definicja w innym pliku: Monaco nie ma jego modelu, więc otwieramy plik w Blue Code.
    try {
      monaco.editor.registerEditorOpener?.({
        openCodeEditor: (_src: any, resource: any, sel: any) => {
          const path = resource?.path;
          if (!path) return false;
          deps.openLocation(path, sel?.startLineNumber ?? sel?.lineNumber ?? 1);
          return true;
        },
      });
    } catch { /* starsze wersje Monaco */ }
  }

  /** Handshake + providery. Zwraca true, gdy LSP działa dla tego języka. */
  async function initialize(language: string): Promise<boolean> {
    if (!BRIDGED_LANGS.includes(language)) return false;
    if (ready.has(language)) return true;
    try {
      await invoke('lsp_initialize', { language, rootPath: deps.getRoot() });
      ready.add(language);
      registerProviders(language);
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

  return { initialize, isActive, didOpen, didChange, didClose };
}

export type LspBridge = ReturnType<typeof createLspBridge>;
