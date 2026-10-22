<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { EditorState, Compartment, Prec, Annotation, type Extension } from '@codemirror/state';
  import {
    EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, highlightSpecialChars,
    drawSelection, dropCursor, rectangularSelection, crosshairCursor,
  } from '@codemirror/view';
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
  import {
    bracketMatching, foldGutter, foldKeymap, indentOnInput, foldAll, unfoldAll, indentUnit,
  } from '@codemirror/language';
  import {
    autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, acceptCompletion, completionStatus,
  } from '@codemirror/autocomplete';
  import { searchKeymap, highlightSelectionMatches, gotoLine, selectNextOccurrence } from '@codemirror/search';
  import { setDiagnostics, lintGutter, lintKeymap, type Diagnostic as CmDiagnostic } from '@codemirror/lint';
  import { abbreviationTracker, expandAbbreviation, emmetConfig } from '@emmetio/codemirror6-plugin';
  import { showMinimap } from '@replit/codemirror-minimap';
  import { indentationMarkers } from '@replit/codemirror-indentation-markers';

  import { blueCodeSettings } from './blueCodeSettings';
  import { loadLanguage, EMMET_SYNTAX } from './cmLanguages';
  import { editorTheme } from './cmThemes';
  import { completionSources, keywordHover } from './cmCompletions';
  import { canFormat, formatSource } from './formatter';
  import type { EditorHandle, EditorMarker } from './editorHandle';
  import type { LspBridge } from './lspBridge';

  interface Props {
    language: string;
    value: string;
    theme?: string;
    fontSize?: number;
    /** Optional LSP bridge; completions / hover / go-to-definition come from it once it is active. */
    lsp?: LspBridge | null;
    /** Changes whenever LSP availability changes, so the editor re-reads `lsp.isActive()`. */
    lspVersion?: unknown;
    /** Hide the minimap / gutter extras (Notepad uses a lighter editor). */
    minimal?: boolean;
    onchange?: (value: string) => void;
    onready?: (handle: EditorHandle) => void;
    oncursor?: (pos: { line: number; col: number }) => void;
    onsave?: () => void;
    onsaveall?: () => void;
    oncommandpalette?: () => void;
    onquickopen?: () => void;
  }

  let {
    language, value, theme = 'blue-dark', fontSize = 13, lsp = null, lspVersion = 0, minimal = false,
    onchange, onready, oncursor, onsave, onsaveall, oncommandpalette, onquickopen,
  }: Props = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | undefined;
  let handle: EditorHandle;

  const External = Annotation.define<boolean>();
  const cLanguage = new Compartment();
  const cTheme = new Compartment();
  const cFont = new Compartment();
  const cView = new Compartment();      // tab size / wrapping
  const cMinimap = new Compartment();
  const cComplete = new Compartment();  // completion sources + LSP extensions + Emmet
  let lastEmitted = value;

  // ── markers (diagnostics) per source ──────────────────────────────────
  const markerSets = new Map<string, EditorMarker[]>();
  function pushMarkers() {
    if (!view) return;
    const doc = view.state.doc;
    const all: CmDiagnostic[] = [];
    for (const [source, list] of markerSets) {
      for (const m of list) {
        const sl = Math.min(Math.max(m.startLine, 1), doc.lines), el = Math.min(Math.max(m.endLine, 1), doc.lines);
        const a = doc.line(sl), b = doc.line(el);
        const from = Math.min(a.from + Math.max(m.startCol - 1, 0), a.to);
        let to = Math.min(b.from + Math.max(m.endCol - 1, 0), b.to);
        if (to <= from) to = Math.min(from + 1, doc.length);
        all.push({ from, to, severity: m.severity, message: m.message, source: m.source ?? source });
      }
    }
    view.dispatch(setDiagnostics(view.state, all));
  }

  // ── Emmet + completion bundle (depends on language / LSP state) ───────
  function completionBundle(lang: string): Extension {
    const lspOn = !!lsp && lsp.isActive(lang);
    const sources = [...(lspOn ? [lsp!.completionSource(lang)] : []), ...completionSources(lang)];
    const emmetSyntax = EMMET_SYNTAX[lang];
    const parts: Extension[] = [
      autocompletion({ override: sources, activateOnTyping: true, icons: true }),
      keywordHover(lang, () => !!lsp && lsp.isActive(lang)),
    ];
    if (lspOn) parts.push(lsp!.hover(lang), lsp!.definition(lang));
    if (emmetSyntax) {
      parts.push(
        emmetConfig.of({ syntax: emmetSyntax as any, autocompleteTab: true, markTagPairs: true, autoRenameTags: true }),
        abbreviationTracker(),
      );
    }
    // Tab: accept an open completion → expand an Emmet abbreviation (`!`, `div.a>p*3`, `lorem` …)
    // → otherwise indent. (A running snippet's own Tab handler has higher precedence.)
    parts.push(Prec.high(keymap.of([{
      key: 'Tab',
      run: (v) => {
        if (completionStatus(v.state) === 'active' && acceptCompletion(v)) return true;
        if (emmetSyntax && expandAbbreviation(v)) return true;
        return false;
      },
    }])));
    return parts;
  }

  function viewSettings(tab: number, wrap: boolean): Extension {
    return [EditorState.tabSize.of(tab), indentUnit.of(' '.repeat(tab)), wrap ? EditorView.lineWrapping : []];
  }

  function minimapExt(on: boolean): Extension {
    if (!on || minimal) return [];
    return showMinimap.compute(['doc'], () => ({
      create: () => ({ dom: document.createElement('div') }),
      displayText: 'blocks', showOverlay: 'mouse-over',
    }));
  }

  function fontTheme(px: number): Extension {
    return EditorView.theme({ '&': { fontSize: `${px}px` }, '.cm-scroller': { lineHeight: '1.5' } });
  }

  function makeHandle(): EditorHandle {
    const v = () => view!;
    return {
      get view() { return view!; },
      focus: () => view?.focus(),
      getValue: () => view?.state.doc.toString() ?? '',
      revealLine(line: number) {
        if (!view) return;
        const ln = view.state.doc.line(Math.min(Math.max(line, 1), view.state.doc.lines));
        view.dispatch({ selection: { anchor: ln.from }, effects: EditorView.scrollIntoView(ln.from, { y: 'center' }) });
      },
      goToLine: () => { if (view) { gotoLine(v()); view.focus(); } },
      foldAll: () => { if (view) foldAll(v()); },
      unfoldAll: () => { if (view) unfoldAll(v()); },
      selectNextOccurrence: () => { if (view) selectNextOccurrence(v()); },
      async format() {
        if (!view || !canFormat(language)) return false;
        const text = view.state.doc.toString();
        const out = await formatSource(language, text, { tabSize: $blueCodeSettings.tabSize });
        if (!view || out === text) return true;
        const head = Math.min(view.state.selection.main.head, out.length);
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: out }, selection: { anchor: head }, userEvent: 'input.format' });
        return true;
      },
      setMarkers(source, markers) {
        if (markers.length === 0) markerSets.delete(source); else markerSets.set(source, markers);
        pushMarkers();
      },
    };
  }

  onMount(() => {
    if (!container) return;
    const cmds = keymap.of([
      { key: 'Mod-s', preventDefault: true, run: () => { onsave?.(); return true; } },
      { key: 'Mod-Shift-s', preventDefault: true, run: () => { onsaveall?.(); return true; } },
      { key: 'Mod-Shift-p', preventDefault: true, run: () => { oncommandpalette?.(); return true; } },
      { key: 'Mod-p', preventDefault: true, run: () => { onquickopen?.(); return true; } },
      { key: 'Mod-g', preventDefault: true, run: (v) => { gotoLine(v); return true; } },
      { key: 'Shift-Alt-f', preventDefault: true, run: () => { void handle.format().catch(() => {}); return true; } },
    ]);

    const s = $blueCodeSettings;
    const state = EditorState.create({
      doc: value,
      extensions: [
        Prec.highest(cmds),
        lineNumbers(), highlightActiveLineGutter(), highlightSpecialChars(), history(), foldGutter(),
        drawSelection(), dropCursor(), EditorState.allowMultipleSelections.of(true), indentOnInput(),
        bracketMatching(), closeBrackets(), rectangularSelection(), crosshairCursor(),
        highlightActiveLine(), highlightSelectionMatches(), lintGutter(), indentationMarkers(),
        keymap.of([
          ...closeBracketsKeymap, ...completionKeymap, ...searchKeymap, ...historyKeymap,
          ...foldKeymap, ...lintKeymap, indentWithTab, ...defaultKeymap,
        ]),
        cLanguage.of([]),
        cTheme.of(editorTheme(theme)),
        cFont.of(fontTheme(fontSize)),
        cView.of(viewSettings(s.tabSize, s.wordWrap)),
        cMinimap.of(minimapExt(s.minimap)),
        cComplete.of(completionBundle(language)),
        EditorView.updateListener.of((u) => {
          if (u.docChanged && !u.transactions.some((t) => t.annotation(External))) {
            lastEmitted = u.state.doc.toString();
            onchange?.(lastEmitted);
          }
          if (u.selectionSet || u.docChanged) {
            const head = u.state.selection.main.head;
            const ln = u.state.doc.lineAt(head);
            oncursor?.({ line: ln.number, col: head - ln.from + 1 });
          }
        }),
      ],
    });
    view = new EditorView({ state, parent: container });
    handle = makeHandle();
    applyLanguage(language);
    oncursor?.({ line: 1, col: 1 });
    onready?.(handle);
  });

  onDestroy(() => { view?.destroy(); view = undefined; });

  function applyLanguage(lang: string) {
    loadLanguage(lang).then((ext) => {
      if (!view || lang !== language) return;
      view.dispatch({ effects: cLanguage.reconfigure(ext ?? []) });
    });
  }

  // External value (tab switch inside the same editor, file reloaded from disk …)
  $effect(() => {
    const v = value;
    untrack(() => {
      if (!view || v === lastEmitted) return;
      if (view.state.doc.toString() === v) { lastEmitted = v; return; }
      lastEmitted = v;
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: v }, annotations: External.of(true) });
    });
  });

  // Language changes (file renamed to another extension …)
  $effect(() => {
    const lang = language;
    untrack(() => {
      if (!view) return;
      applyLanguage(lang);
      view.dispatch({ effects: cComplete.reconfigure(completionBundle(lang)) });
    });
  });

  // LSP became (un)available
  $effect(() => {
    void lspVersion; void lsp;
    untrack(() => { if (view) view.dispatch({ effects: cComplete.reconfigure(completionBundle(language)) }); });
  });

  $effect(() => { const t = theme; untrack(() => view?.dispatch({ effects: cTheme.reconfigure(editorTheme(t)) })); });
  $effect(() => { const f = fontSize; untrack(() => view?.dispatch({ effects: cFont.reconfigure(fontTheme(f)) })); });
  $effect(() => {
    const s = $blueCodeSettings;
    untrack(() => view?.dispatch({ effects: [
      cView.reconfigure(viewSettings(s.tabSize, s.wordWrap)),
      cMinimap.reconfigure(minimapExt(s.minimap)),
    ] }));
  });
</script>

<div bind:this={container} class="w-full h-full overflow-hidden"></div>
