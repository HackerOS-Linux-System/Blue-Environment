import { describe, it, expect } from 'vitest';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { snippet } from '@codemirror/autocomplete';
import { ensureSyntaxTree } from '@codemirror/language';
import { abbreviationTracker, expandAbbreviation, emmetConfig } from '@emmetio/codemirror6-plugin';
import { loadLanguage, supportedLanguages } from './cmLanguages';
import { lspSnippetToCm } from './snippetSyntax';
import { buildCompletions } from './cmCompletions';
import { canFormat, formatSource } from './formatter';
import { getLang } from './languageMap';

// jsdom has no layout engine; CodeMirror's measure pass needs these.
const emptyRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} }) as any;
(Range.prototype as any).getClientRects ??= emptyRects;
(Range.prototype as any).getBoundingClientRect ??= () => ({ x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom: 0 });

describe('language loading', () => {
  it('loads every supported language', async () => {
    for (const id of supportedLanguages()) {
      expect(await loadLanguage(id), id).toBeTruthy();
    }
  });
  it('tokenizes HackerOS languages without throwing', async () => {
    const samples: Record<string, string> = {
      hsharp: 'fn main() -> void {\n  let x = 0x1F; ;; note\n  "str\\n"\n}',
      hackerlang: '~> ::run\n%var = "a $(ls) @x"\n;; c',
      hackerscript: 'fun a() { let s = "x" } !> block <!',
      hk: '[sec]\nkey => "v ${x}"\n! c',
      blue: '[a]\nb = 1.2.3',
      hacker: '[s]\nk: "v ${x}"\n# c',
    };
    for (const [id, doc] of Object.entries(samples)) {
      const ext = (await loadLanguage(id))!;
      const state = EditorState.create({ doc, extensions: [ext] });
      const tree = ensureSyntaxTree(state, doc.length, 5000);
      expect(tree, id).toBeTruthy();
      expect(tree!.length).toBe(doc.length);
    }
  });
  it('maps new extensions', () => {
    expect(getLang('a.kt')).toBe('kotlin');
    expect(getLang('x.svelte')).toBe('svelte');
    expect(getLang('x.toml')).toBe('toml');
  });
});

describe('Emmet + snippets', () => {
  async function htmlView(doc: string) {
    const lang = (await loadLanguage('html'))!;
    const parent = document.createElement('div');
    document.body.appendChild(parent);
    return new EditorView({
      parent,
      state: EditorState.create({
        doc, selection: { anchor: doc.length },
        extensions: [lang, emmetConfig.of({ syntax: 'html' as any }), abbreviationTracker()],
      }),
    });
  }
  it('"!" + Tab generates the HTML5 template', async () => {
    const view = await htmlView('!');
    expect(expandAbbreviation(view)).toBe(true);
    const out = view.state.doc.toString();
    expect(out).toContain('<!DOCTYPE html>');
    expect(out).toContain('<meta name="viewport"');
    expect(out).toContain('<body>');
  });
  it('div.box>ul>li*3 expands', async () => {
    const view = await htmlView('div.box>ul>li*3');
    expandAbbreviation(view);
    const out = view.state.doc.toString();
    expect(out).toContain('<div class="box">');
    expect(out.match(/<li>/g)?.length).toBe(3);
  });
  it('snippet templates (incl. $0 final cursor) apply', () => {
    const view = new EditorView({ parent: document.body.appendChild(document.createElement('div')), state: EditorState.create({ doc: '' }) });
    snippet(lspSnippetToCm('fn ${1:name}() {\n\t$0\n}'))(view, null, 0, 0);
    const out = view.state.doc.toString();
    expect(out).toMatch(/^fn name\(\) \{\n\s+\n\}$/);
    expect(out).not.toContain('$0');
  });
  it('offers the "!" completion for HTML and many language snippets', () => {
    expect(buildCompletions('html').some((c) => c.label === '!')).toBe(true);
    for (const l of ['java', 'kotlin', 'swift', 'csharp', 'php', 'sql', 'shell', 'ruby', 'lua'])
      expect(buildCompletions(l).length, l).toBeGreaterThan(5);
  });
});

describe('formatter', () => {
  it('formats JSON, CSS and TypeScript with Prettier', async () => {
    expect(canFormat('json')).toBe(true);
    expect(await formatSource('json', '{"a":1,"b":[1,2]}', { tabSize: 2 })).toBe('{ "a": 1, "b": [1, 2] }\n');
    expect(await formatSource('css', 'a{color:red}', { tabSize: 2 })).toContain('color: red;');
    expect(await formatSource('typescript', 'const a:number=1', { tabSize: 2 })).toContain('const a: number = 1;');
    expect(canFormat('rust')).toBe(false);
  });
});
