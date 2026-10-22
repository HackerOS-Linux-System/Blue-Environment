import { HACKER_EXT_TO_LANG } from './hackerLanguages';

const EXT_TO_LANG: Record<string, string> = {
    html: 'html', css: 'css', scss: 'scss', less: 'less',
    js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
    ts: 'typescript', tsx: 'typescript',
    rs: 'rust', go: 'go', py: 'python',
    rb: 'ruby', php: 'php', lua: 'lua',
    c: 'c', cpp: 'cpp', h: 'c', hpp: 'cpp', cc: 'cpp',
    java: 'java', kt: 'kotlin', swift: 'swift', cs: 'csharp',
    json: 'json', yaml: 'yaml', yml: 'yaml',
    toml: 'toml', xml: 'xml', md: 'markdown', mdx: 'markdown',
    sh: 'shell', bash: 'shell', zsh: 'shell', fish: 'shell',
    sql: 'sql', graphql: 'graphql', dockerfile: 'dockerfile',
    vue: 'vue', svelte: 'svelte', svg: 'xml', xhtml: 'html', htm: 'html', xsl: 'xml', plist: 'xml',
    sass: 'sass', php5: 'php', phtml: 'php', jsonc: 'json', json5: 'json', webmanifest: 'json',
    markdown: 'markdown', rmd: 'markdown',
    scala: 'scala', sc: 'scala', dart: 'dart', m: 'objectivec', mm: 'objectivec',
    pl: 'perl', pm: 'perl', r: 'r', ps1: 'powershell', psm1: 'powershell',
    hs: 'haskell', jl: 'julia', groovy: 'groovy', gradle: 'groovy', clj: 'clojure', cljs: 'clojure',
    erl: 'erlang', conf: 'properties', cfg: 'properties', properties: 'properties', env: 'properties',
    cmake: 'cmake', proto: 'protobuf', diff: 'diff', patch: 'diff', tex: 'stex',
    mk: 'makefile', ino: 'cpp', cxx: 'cpp', hh: 'cpp', hxx: 'cpp', ixx: 'cpp', kts: 'kotlin',
    // HackerOS family + Blue's own format (tokenizers: hackerLanguages.ts)
    ...HACKER_EXT_TO_LANG,
};

export function getLang(path: string): string {
    const base = path.split('/').pop()?.toLowerCase() ?? '';
    if (base === 'dockerfile') return 'dockerfile';
    if (base === 'makefile' || base === 'gnumakefile') return 'makefile';
    if (base === 'cmakelists.txt') return 'cmake';
    if (base === 'nginx.conf') return 'nginx';
    if (base === '.bashrc' || base === '.zshrc' || base === '.profile' || base === 'pkgbuild') return 'shell';
    // `.h#` — the extension contains a '#', so split on the LAST dot only.
    const ext = base.includes('.') ? base.slice(base.lastIndexOf('.') + 1) : '';
    return EXT_TO_LANG[ext] || 'plaintext';
}

/** Languages for which we attempt to start a real LSP server (see SystemBridge.startLanguageServer). */
export const LSP_LANGS = ['typescript', 'javascript', 'rust', 'python', 'go', 'cpp', 'c'];

/** Languages whose text has an HTML-like structure (Emmet, tag auto-close). */
export const MARKUP_LANGS = ['html', 'vue', 'svelte', 'xml'];

/** File patterns indexed during full-text search (avoids scanning binary blobs). */
export const SEARCHABLE_EXT_RE = /\.(ts|tsx|js|jsx|mjs|cjs|rs|py|go|rb|php|lua|c|cpp|h|hpp|java|kt|kts|swift|cs|json|toml|yaml|yml|xml|svg|md|sh|bash|sql|html|htm|css|scss|sass|less|vue|svelte|dart|scala|pl|r|ps1|hs|jl|gradle|cmake|h#|hl|hcs|hk|hacker|blue)$/;
