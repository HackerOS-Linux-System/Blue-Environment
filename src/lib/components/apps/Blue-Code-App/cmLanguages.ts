import type { Extension } from '@codemirror/state';
import { StreamLanguage, type StreamParser } from '@codemirror/language';
import { hackerLanguage, HACKER_LANG_IDS, HACKER_LANG_LABEL } from './hackerLanguages';

/**
 * Lazy language loader for the CodeMirror 6 editor. Each grammar is its own
 * dynamic import, so only the languages a person actually opens are loaded.
 *
 * ids are the ones produced by languageMap.getLang().
 */

type Loader = () => Promise<Extension>;

const legacy = (load: () => Promise<StreamParser<unknown>>): Loader => async () => StreamLanguage.define(await load());

const LOADERS: Record<string, Loader> = {
    // ── first-class Lezer grammars ───────────────────────────────────────
    javascript: async () => (await import('@codemirror/lang-javascript')).javascript({ jsx: true }),
    typescript: async () => (await import('@codemirror/lang-javascript')).javascript({ jsx: true, typescript: true }),
    html: async () => (await import('@codemirror/lang-html')).html({ autoCloseTags: true, matchClosingTags: true }),
    svelte: async () => (await import('@codemirror/lang-html')).html({ autoCloseTags: true, matchClosingTags: true }),
    vue: async () => (await import('@codemirror/lang-vue')).vue(),
    css: async () => (await import('@codemirror/lang-css')).css(),
    scss: async () => (await import('@codemirror/lang-sass')).sass({ indented: false }),
    sass: async () => (await import('@codemirror/lang-sass')).sass({ indented: true }),
    less: async () => (await import('@codemirror/lang-less')).less(),
    json: async () => (await import('@codemirror/lang-json')).json(),
    markdown: async () => (await import('@codemirror/lang-markdown')).markdown(),
    python: async () => (await import('@codemirror/lang-python')).python(),
    rust: async () => (await import('@codemirror/lang-rust')).rust(),
    c: async () => (await import('@codemirror/lang-cpp')).cpp(),
    cpp: async () => (await import('@codemirror/lang-cpp')).cpp(),
    java: async () => (await import('@codemirror/lang-java')).java(),
    php: async () => (await import('@codemirror/lang-php')).php(),
    sql: async () => (await import('@codemirror/lang-sql')).sql(),
    xml: async () => (await import('@codemirror/lang-xml')).xml(),
    yaml: async () => (await import('@codemirror/lang-yaml')).yaml(),
    go: async () => (await import('@codemirror/lang-go')).go(),

    // ── CodeMirror legacy (stream) modes ─────────────────────────────────
    shell: legacy(async () => (await import('@codemirror/legacy-modes/mode/shell')).shell),
    ruby: legacy(async () => (await import('@codemirror/legacy-modes/mode/ruby')).ruby),
    lua: legacy(async () => (await import('@codemirror/legacy-modes/mode/lua')).lua),
    swift: legacy(async () => (await import('@codemirror/legacy-modes/mode/swift')).swift),
    kotlin: legacy(async () => (await import('@codemirror/legacy-modes/mode/clike')).kotlin),
    csharp: legacy(async () => (await import('@codemirror/legacy-modes/mode/clike')).csharp),
    scala: legacy(async () => (await import('@codemirror/legacy-modes/mode/clike')).scala),
    dart: legacy(async () => (await import('@codemirror/legacy-modes/mode/clike')).dart),
    objectivec: legacy(async () => (await import('@codemirror/legacy-modes/mode/clike')).objectiveC),
    perl: legacy(async () => (await import('@codemirror/legacy-modes/mode/perl')).perl),
    r: legacy(async () => (await import('@codemirror/legacy-modes/mode/r')).r),
    powershell: legacy(async () => (await import('@codemirror/legacy-modes/mode/powershell')).powerShell),
    haskell: legacy(async () => (await import('@codemirror/legacy-modes/mode/haskell')).haskell),
    julia: legacy(async () => (await import('@codemirror/legacy-modes/mode/julia')).julia),
    groovy: legacy(async () => (await import('@codemirror/legacy-modes/mode/groovy')).groovy),
    clojure: legacy(async () => (await import('@codemirror/legacy-modes/mode/clojure')).clojure),
    erlang: legacy(async () => (await import('@codemirror/legacy-modes/mode/erlang')).erlang),
    toml: legacy(async () => (await import('@codemirror/legacy-modes/mode/toml')).toml),
    properties: legacy(async () => (await import('@codemirror/legacy-modes/mode/properties')).properties),
    ini: legacy(async () => (await import('@codemirror/legacy-modes/mode/properties')).properties),
    dockerfile: legacy(async () => (await import('@codemirror/legacy-modes/mode/dockerfile')).dockerFile),
    cmake: legacy(async () => (await import('@codemirror/legacy-modes/mode/cmake')).cmake),
    nginx: legacy(async () => (await import('@codemirror/legacy-modes/mode/nginx')).nginx),
    protobuf: legacy(async () => (await import('@codemirror/legacy-modes/mode/protobuf')).protobuf),
    diff: legacy(async () => (await import('@codemirror/legacy-modes/mode/diff')).diff),
    stex: legacy(async () => (await import('@codemirror/legacy-modes/mode/stex')).stex),
    makefile: legacy(async () => (await import('@codemirror/legacy-modes/mode/shell')).shell),
};

/** Emmet syntax per language id (`!` + Tab, `div.a>p*3` + Tab …). */
export const EMMET_SYNTAX: Record<string, string> = {
    html: 'html', svelte: 'html', vue: 'vue', php: 'html', xml: 'xml',
    css: 'css', scss: 'scss', sass: 'sass', less: 'less',
    javascript: 'jsx', typescript: 'tsx',
};

/** Human labels for the status bar (Hacker family labels come from hackerLanguages). */
export const LANGUAGE_LABELS: Record<string, string> = {
    javascript: 'JavaScript', typescript: 'TypeScript', html: 'HTML', svelte: 'Svelte', vue: 'Vue', css: 'CSS', scss: 'SCSS',
    sass: 'Sass', less: 'Less', json: 'JSON', markdown: 'Markdown', python: 'Python', rust: 'Rust', c: 'C', cpp: 'C++',
    java: 'Java', php: 'PHP', sql: 'SQL', xml: 'XML', yaml: 'YAML', go: 'Go', shell: 'Shell', ruby: 'Ruby', lua: 'Lua',
    swift: 'Swift', kotlin: 'Kotlin', csharp: 'C#', scala: 'Scala', dart: 'Dart', objectivec: 'Objective-C', perl: 'Perl',
    r: 'R', powershell: 'PowerShell', haskell: 'Haskell', julia: 'Julia', groovy: 'Groovy', clojure: 'Clojure',
    erlang: 'Erlang', toml: 'TOML', properties: 'Properties', ini: 'INI', dockerfile: 'Dockerfile', cmake: 'CMake',
    nginx: 'Nginx', protobuf: 'Protobuf', diff: 'Diff', stex: 'LaTeX', makefile: 'Makefile', plaintext: 'Plain Text',
    ...HACKER_LANG_LABEL,
};

export function languageLabel(id: string): string {
    return LANGUAGE_LABELS[id] ?? id;
}

/** All language ids the editor can highlight (used by tests and the picker). */
export function supportedLanguages(): string[] {
    return [...new Set([...Object.keys(LOADERS), ...HACKER_LANG_IDS])];
}

const cache = new Map<string, Promise<Extension | null>>();

/** Resolves the highlighting extension for a language id, or null for plain text / unknown. */
export function loadLanguage(id: string): Promise<Extension | null> {
    let p = cache.get(id);
    if (!p) {
        const hacker = hackerLanguage(id);
        if (hacker) p = Promise.resolve(hacker);
        else if (LOADERS[id]) p = LOADERS[id]().catch(() => null);
        else p = Promise.resolve(null);
        cache.set(id, p);
    }
    return p;
}
