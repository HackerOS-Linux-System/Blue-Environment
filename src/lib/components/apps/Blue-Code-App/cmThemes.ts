import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags as t } from '@lezer/highlight';

/** Palette for one Blue Code editor theme. */
interface Palette {
  dark: boolean;
  bg: string; fg: string; gutter: string; gutterActive: string; lineHl: string; selection: string; cursor: string;
  panel: string; border: string; hover: string;
  keyword: string; control: string; string: string; escape: string; comment: string; number: string; type: string;
  cls: string; fn: string; variable: string; punct: string; tag: string; attr: string; prop: string; meta: string;
}

const PALETTES: Record<string, Palette> = {
  'blue-dark': {
    dark: true, bg: '#0f172a', fg: '#e2e8f0', gutter: '#334155', gutterActive: '#60a5fa', lineHl: '#1e293b66',
    selection: '#1d4ed8aa', cursor: '#60a5fa', panel: '#0f172a', border: '#1e40af', hover: '#1e3a8a',
    keyword: '#60a5fa', control: '#a78bfa', string: '#34d399', escape: '#6ee7b7', comment: '#64748b', number: '#fb923c',
    type: '#38bdf8', cls: '#fbbf24', fn: '#93c5fd', variable: '#e2e8f0', punct: '#94a3b8', tag: '#60a5fa', attr: '#93c5fd',
    prop: '#cbd5e1', meta: '#f472b6',
  },
  'blue-light': {
    dark: false, bg: '#f8fafc', fg: '#1e293b', gutter: '#94a3b8', gutterActive: '#1d4ed8', lineHl: '#e2e8f099',
    selection: '#bfdbfe', cursor: '#1d4ed8', panel: '#f1f5f9', border: '#93c5fd', hover: '#dbeafe',
    keyword: '#1d4ed8', control: '#7c3aed', string: '#059669', escape: '#047857', comment: '#94a3b8', number: '#d97706',
    type: '#0284c7', cls: '#b45309', fn: '#7c3aed', variable: '#1e293b', punct: '#64748b', tag: '#1d4ed8', attr: '#0369a1',
    prop: '#334155', meta: '#be185d',
  },
  'blue-ocean': {
    dark: true, bg: '#023047', fg: '#caf0f8', gutter: '#2a6080', gutterActive: '#00b4d8', lineHl: '#03405f99',
    selection: '#0077b6aa', cursor: '#00b4d8', panel: '#023047', border: '#0077b6', hover: '#03405f',
    keyword: '#00b4d8', control: '#90e0ef', string: '#90e0ef', escape: '#caf0f8', comment: '#4a7c9e', number: '#ffb703',
    type: '#48cae4', cls: '#ffb703', fn: '#caf0f8', variable: '#caf0f8', punct: '#7fb7d4', tag: '#00b4d8', attr: '#48cae4',
    prop: '#ade8f4', meta: '#ffb703',
  },
};

export const THEME_IDS = Object.keys(PALETTES);

function build(p: Palette): Extension {
  const view = EditorView.theme({
    '&': { color: p.fg, backgroundColor: p.bg, height: '100%' },
    '.cm-content': { caretColor: p.cursor, fontFamily: 'JetBrains Mono, Fira Code, monospace' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: p.cursor },
    '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': { backgroundColor: p.selection },
    '.cm-activeLine': { backgroundColor: p.lineHl },
    '.cm-gutters': { backgroundColor: p.bg, color: p.gutter, border: 'none' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: p.gutterActive },
    '.cm-foldPlaceholder': { backgroundColor: p.hover, border: 'none', color: p.fg },
    '.cm-matchingBracket': { backgroundColor: p.selection, outline: `1px solid ${p.border}` },
    '.cm-tooltip': { backgroundColor: p.panel, border: `1px solid ${p.border}`, color: p.fg },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': { backgroundColor: p.hover, color: p.fg },
    '.cm-completionDetail': { color: p.punct },
    '.cm-panels': { backgroundColor: p.panel, color: p.fg },
    '.cm-panels.cm-panels-top': { borderBottom: `1px solid ${p.border}` },
    '.cm-searchMatch': { backgroundColor: p.selection },
    '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: p.hover, outline: `1px solid ${p.border}` },
    '.cm-textfield': { backgroundColor: p.bg, border: `1px solid ${p.border}`, color: p.fg },
    '.cm-button': { backgroundImage: 'none', backgroundColor: p.hover, border: `1px solid ${p.border}`, color: p.fg },
    '.cm-scroller': { overflow: 'auto', scrollbarWidth: 'thin' },
    '.cm-lint-marker-error': { content: '"●"' },
    '.cm-diagnostic': { color: p.fg },
    '.cm-minimap-gutter': { backgroundColor: p.bg },
    '.cm-emmet-preview, .cm-emmetPreview': { backgroundColor: p.panel },
  }, { dark: p.dark });

  const highlight = HighlightStyle.define([
    { tag: [t.keyword, t.modifier, t.operatorKeyword], color: p.keyword, fontWeight: 'bold' },
    { tag: [t.controlKeyword, t.moduleKeyword], color: p.control, fontWeight: 'bold' },
    { tag: [t.string, t.special(t.string), t.regexp], color: p.string },
    { tag: [t.escape], color: p.escape },
    { tag: [t.comment, t.lineComment, t.blockComment], color: p.comment, fontStyle: 'italic' },
    { tag: t.docComment, color: p.comment, fontStyle: 'italic' },
    { tag: [t.number, t.bool, t.null, t.atom, t.integer, t.float], color: p.number },
    { tag: [t.typeName, t.standard(t.typeName)], color: p.type },
    { tag: [t.className, t.namespace], color: p.cls },
    { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: p.fn },
    { tag: [t.variableName, t.name], color: p.variable },
    { tag: [t.definition(t.variableName)], color: p.variable, fontWeight: '600' },
    { tag: [t.special(t.variableName), t.self], color: p.meta },
    { tag: [t.propertyName, t.labelName], color: p.prop },
    { tag: [t.punctuation, t.separator, t.bracket, t.angleBracket, t.operator, t.derefOperator], color: p.punct },
    { tag: [t.tagName], color: p.tag },
    { tag: [t.attributeName], color: p.attr },
    { tag: [t.attributeValue], color: p.string },
    { tag: [t.meta, t.annotation, t.processingInstruction], color: p.meta },
    { tag: [t.heading], color: p.keyword, fontWeight: 'bold' },
    { tag: [t.link, t.url], color: p.type, textDecoration: 'underline' },
    { tag: t.emphasis, fontStyle: 'italic' },
    { tag: t.strong, fontWeight: 'bold' },
    { tag: t.strikethrough, textDecoration: 'line-through' },
    { tag: t.invalid, color: '#f87171' },
    { tag: t.inserted, color: p.string }, { tag: t.deleted, color: '#f87171' }, { tag: t.changed, color: p.number },
  ]);
  return [view, syntaxHighlighting(highlight)];
}

const cache = new Map<string, Extension>();

/** Editor theme extension. Unknown names (e.g. Monaco's `vs-dark`) fall back to Blue Dark. */
export function editorTheme(name: string): Extension {
  const key = name in PALETTES ? name : name === 'vs' ? 'blue-light' : 'blue-dark';
  let ext = cache.get(key);
  if (!ext) { ext = build(PALETTES[key]); cache.set(key, ext); }
  return ext;
}
