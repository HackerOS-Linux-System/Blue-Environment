import { snippet, completeAnyWord, type Completion, type CompletionContext, type CompletionResult, type CompletionSource } from '@codemirror/autocomplete';
import { hoverTooltip, type Tooltip } from '@codemirror/view';
import { KW, SNIPPETS } from './completionData';
import { EXTRA_KW, EXTRA_SNIPPETS } from './snippetsExtra';
import { lspSnippetToCm } from './snippetSyntax';

/** Merged keyword / snippet banks (original Blue Code ones + the new languages). */
export function keywordsFor(language: string): string[] {
  return [...(KW[language] ?? []), ...(EXTRA_KW[language] ?? [])];
}
export function snippetsFor(language: string) {
  return [...(SNIPPETS[language] ?? []), ...(EXTRA_SNIPPETS[language] ?? [])];
}

/** Builds the (static) completion list for one language; cached by the caller. */
export function buildCompletions(language: string): Completion[] {
  const out: Completion[] = [];
  for (const kw of new Set(keywordsFor(language))) {
    out.push({ label: kw, type: 'keyword', detail: `${language} keyword`, boost: -1 });
  }
  for (const s of snippetsFor(language)) {
    out.push({
      label: s.label, detail: s.detail, type: 'snippet',
      info: s.documentation, boost: 2,
      // `snippet()` understands the same ${1:placeholder} / $0 syntax as VS Code.
      apply: snippet(lspSnippetToCm(s.insertText)),
    });
  }
  return out;
}

const cache = new Map<string, Completion[]>();

/** Word-prefix match; `!` is allowed so the HTML `!` snippet is offered too. */
function wordBefore(ctx: CompletionContext) {
  return ctx.matchBefore(/[!\w$#.:@-]*/);
}

/** Completion source: keywords + snippets of the language (words in the document come from completeAnyWord). */
export function blueCompletionSource(language: string): CompletionSource {
  return (ctx: CompletionContext): CompletionResult | null => {
    const word = wordBefore(ctx);
    if (!word || (word.from === word.to && !ctx.explicit)) return null;
    let options = cache.get(language);
    if (!options) { options = buildCompletions(language); cache.set(language, options); }
    if (options.length === 0) return null;
    return { from: word.from, options, validFor: /^[!\w$#.:@-]*$/ };
  };
}

/** Sources to put in `autocompletion({ override })` for a language. */
export function completionSources(language: string): CompletionSource[] {
  return [blueCompletionSource(language), completeAnyWord];
}

// ── Hover docs for a handful of keywords (kept from the Monaco version) ──
const DOCS: Record<string, string> = {
  async: 'Marks a function as asynchronous; it returns a Promise.',
  await: 'Pauses execution until the Promise resolves.',
  const: 'Block-scoped constant; cannot be reassigned.',
  let: 'Block-scoped variable.',
  fn: 'Rust function keyword.',
  match: 'Rust pattern-matching expression (exhaustive).',
  mut: 'Rust keyword: marks a variable or reference as mutable.',
  def: 'Python function definition keyword.',
  yield: 'Produces a value from a generator function.',
  defer: 'Go: execute a statement when the surrounding function returns.',
  go: 'Go: launches a goroutine (lightweight concurrent function).',
};

const DOC_LANGS = new Set(['javascript', 'typescript', 'python', 'rust', 'go']);

/** Hover tooltip with the keyword docs. Skipped when an LSP server provides its own hover. */
export function keywordHover(language: string, lspActive: () => boolean) {
  if (!DOC_LANGS.has(language)) return [];
  const kws = new Set(keywordsFor(language));
  return hoverTooltip((view, pos): Tooltip | null => {
    if (lspActive()) return null;
    const line = view.state.doc.lineAt(pos);
    const re = /[A-Za-z_]\w*/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(line.text))) {
      const from = line.from + m.index, to = from + m[0].length;
      if (pos >= from && pos <= to) {
        const doc = DOCS[m[0]];
        if (!doc || !kws.has(m[0])) return null;
        return {
          pos: from, end: to, above: true,
          create() {
            const dom = document.createElement('div');
            dom.style.cssText = 'padding:4px 8px;font-size:12px;max-width:320px';
            const b = document.createElement('b'); b.textContent = m![0];
            dom.append(b, ' — ' + doc);
            return { dom };
          },
        };
      }
    }
    return null;
  });
}
