export const HACKER_LANG_IDS = ['hsharp', 'hackerlang', 'hackerscript', 'hk', 'hacker', 'blue'] as const;

/** extension (no dot) → Monaco language id. Merged into languageMap.ts. */
export const HACKER_EXT_TO_LANG: Record<string, string> = {
    'h#': 'hsharp',
    hl: 'hackerlang',
    hcs: 'hackerscript',
    hk: 'hk',
    hacker: 'hacker',
    blue: 'blue',
};

/** Human label for the status bar. */
export const HACKER_LANG_LABEL: Record<string, string> = {
    hsharp: 'H#', hackerlang: 'Hacker Lang', hackerscript: 'HackerScript',
    hk: 'HK', hacker: 'Hacker', blue: 'Blue',
};

let registered = false;

export function registerHackerLanguages(monaco: any) {
    if (registered) return;
    registered = true;
    const langs = monaco.languages;

    const common = {
        brackets: [['{', '}'], ['[', ']'], ['(', ')']] as [string, string][],
        autoClosingPairs: [
            { open: '{', close: '}' }, { open: '[', close: ']' }, { open: '(', close: ')' },
            { open: '"', close: '"', notIn: ['string'] }, { open: "'", close: "'", notIn: ['string'] },
        ],
        surroundingPairs: [{ open: '{', close: '}' }, { open: '[', close: ']' }, { open: '(', close: ')' }, { open: '"', close: '"' }],
    };

    // ── H# ──────────────────────────────────────────────────────────────
    langs.register({ id: 'hsharp', extensions: ['.h#'], aliases: ['H#', 'H-Sharp'] });
    langs.setLanguageConfiguration('hsharp', { ...common, comments: { lineComment: ';;' } });
    langs.setMonarchTokensProvider('hsharp', {
        defaultToken: '',
        keywords: ['fn', 'func', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait', 'if', 'else', 'elif', 'match', 'while', 'for', 'in', 'loop', 'break', 'continue', 'return', 'use', 'import', 'from', 'as', 'pub', 'mod', 'type', 'unsafe', 'async', 'await', 'defer', 'extern', 'static', 'self', 'Self'],
        types: ['int', 'i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'f32', 'f64', 'bool', 'char', 'str', 'string', 'void', 'usize', 'isize', 'Vec', 'Option', 'Result', 'Box'],
        constants: ['true', 'false', 'null', 'none', 'None', 'Some', 'Ok', 'Err'],
        tokenizer: {
            root: [
                [/;;.*$/, 'comment'],
                [/\/\*/, 'comment', '@block'],
                [/#\[[^\]]*\]/, 'annotation'],
                [/"/, 'string', '@string'],
                [/'(?:[^'\\]|\\.)'/, 'string'],
                [/\b0x[0-9a-fA-F_]+\b/, 'number.hex'],
                [/\b\d[\d_]*(\.\d+)?\b/, 'number'],
                [/[A-Z][A-Za-z0-9_]*/, { cases: { '@constants': 'constant', '@types': 'type', '@default': 'type.identifier' } }],
                [/[a-z_][A-Za-z0-9_]*(?=\s*\()/, { cases: { '@keywords': 'keyword', '@default': 'function' } }],
                [/[a-z_][A-Za-z0-9_]*/, { cases: { '@keywords': 'keyword', '@types': 'type', '@constants': 'constant', '@default': 'identifier' } }],
                [/->|=>|::|\.\.=?|[+\-*/%&|^!<>=]=?|&&|\|\|/, 'operator'],
                [/[{}()\[\]]/, '@brackets'],
            ],
            string: [[/[^\\"]+/, 'string'], [/\\./, 'string.escape'], [/"/, 'string', '@pop']],
            block: [[/[^*/]+/, 'comment'], [/\*\//, 'comment', '@pop'], [/[*/]/, 'comment']],
        },
    });

    // ── Hacker Lang (.hl) ───────────────────────────────────────────────
    langs.register({ id: 'hackerlang', extensions: ['.hl'], aliases: ['Hacker Lang', 'hl'] });
    langs.setLanguageConfiguration('hackerlang', { ...common, comments: { lineComment: ';;' } });
    langs.setMonarchTokensProvider('hackerlang', {
        defaultToken: '',
        keywords: ['done', 'using', 'if', 'else', 'elif', 'for', 'while', 'in', 'do', 'then', 'end', 'fn', 'return', 'break', 'continue', 'import', 'spawn', 'wait'],
        tokenizer: {
            root: [
                [/^#!.*$/, 'comment.shebang'],
                [/;;;.*$/, 'comment.doc'],
                [/\/\/\/.*$/, 'comment.doc'],
                [/;;.*$/, 'comment'],
                [/\/\/.*$/, 'comment'],
                [/^\s*~>/, 'keyword.print'],
                [/::[a-zA-Z_][\w]*/, 'function'],                 // ::bold ::green ::nl ::exists …
                [/\$\(/, { token: 'variable.predefined', next: '@interp' }],
                [/@[A-Za-z_][\w]*/, 'variable'],                  // @var
                [/\$[A-Za-z_][\w]*/, 'variable'],
                [/%\s*[A-Za-z_][\w]*/, 'variable.name'],          // % name = value
                [/\?~|\?|:\*\*?|\*>|\*--|\/>|<<|>>|=>|\|>|\|\||&&|->|-->|--|~>/, 'keyword.operator'],
                [/"/, 'string', '@string'],
                [/'[^']*'/, 'string'],
                [/\b\d+(\.\d+)?\b/, 'number'],
                [/[a-zA-Z_][\w-]*/, { cases: { '@keywords': 'keyword', '@default': 'identifier' } }],
                [/[{}()\[\]]/, '@brackets'],
                [/[|>&]/, 'operator'],
            ],
            interp: [[/\)/, 'variable.predefined', '@pop'], [/@?[\w]+/, 'variable'], [/./, 'operator']],
            string: [
                [/\$\([^)]*\)/, 'variable'],
                [/@[A-Za-z_]\w*/, 'variable'],
                [/[^\\"$@]+/, 'string'], [/\\./, 'string.escape'], [/[$@]/, 'string'],
                [/"/, 'string', '@pop'],
            ],
        },
    });

    // ── HackerScript (.hcs) ─────────────────────────────────────────────
    langs.register({ id: 'hackerscript', extensions: ['.hcs'], aliases: ['HackerScript', 'hcs'] });
    langs.setLanguageConfiguration('hackerscript', {
        ...common, comments: { lineComment: '//', blockComment: ['!>', '<!'] },
    });
    langs.setMonarchTokensProvider('hackerscript', {
        defaultToken: '',
        keywords: ['fun', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait', 'if', 'else', 'elif', 'match', 'while', 'for', 'in', 'loop', 'break', 'continue', 'return', 'end', 'get', 'use', 'import', 'from', 'as', 'pub', 'mod', 'type', 'async', 'await', 'defer', 'try', 'catch', 'throw', 'self', 'Self', 'is', 'not', 'and', 'or'],
        modes: ['direct', 'manual', 'native', 'region', 'extern', 'static', 'dynamic', 'unsafe'],
        types: ['int', 'float', 'bool', 'str', 'string', 'char', 'void', 'list', 'map', 'any', 'i32', 'i64', 'u32', 'u64', 'f32', 'f64', 'usize'],
        constants: ['true', 'false', 'null', 'none'],
        tokenizer: {
            root: [
                [/!!.*$/, 'comment'],
                [/\/\/.*$/, 'comment'],
                [/!>/, 'comment', '@block'],
                [/\/\*/, 'comment', '@cblock'],
                [/"/, 'string', '@string'],
                [/'(?:[^'\\]|\\.)'/, 'string'],
                [/<[A-Za-z_][\w:.\/-]*>/, 'string.include'],       // get <std/io>
                [/\b0x[0-9a-fA-F_]+\b/, 'number.hex'],
                [/\b\d[\d_]*(\.\d+)?\b/, 'number'],
                [/[A-Za-z_]\w*(?=\s*\()/, { cases: { '@keywords': 'keyword', '@default': 'function' } }],
                [/[A-Za-z_]\w*/, { cases: { '@keywords': 'keyword', '@modes': 'keyword.flow', '@types': 'type', '@constants': 'constant', '@default': 'identifier' } }],
                [/->|=>|::|\.\.=?|[+\-*/%&|^!<>=]=?|&&|\|\|/, 'operator'],
                [/[{}()\[\]]/, '@brackets'],
            ],
            string: [[/\$\{[^}]*\}/, 'variable'], [/[^\\"$]+/, 'string'], [/\\./, 'string.escape'], [/\$/, 'string'], [/"/, 'string', '@pop']],
            block: [[/<!/, 'comment', '@pop'], [/[^<]+/, 'comment'], [/</, 'comment']],
            cblock: [[/[^*/]+/, 'comment'], [/\*\//, 'comment', '@pop'], [/[*/]/, 'comment']],
        },
    });

    // ── HK (.hk) and Blue (.blue manifest) — same family ────────────────
    const hkTokenizer = {
        defaultToken: '',
        tokenizer: {
            root: [
                [/^\s*!.*$/, 'comment'],                                   // ! comment
                [/^\s*(#|;).*$/, 'comment'],
                [/^\s*\[\s*[^\]]*\]/, 'type'],                              // [section] / [section.sub]
                [/^(\s*)([A-Za-z_][\w.\-]*)(\s*)(=>|=)/, ['white', 'variable.name', 'white', 'operator']],   // key => value
                [/^(\s*)(->|-->)(\s*)([A-Za-z_][\w.\-]*)/, ['white', 'operator', 'white', 'variable.name']], // nested -> key
                [/\$\{[^}]*\}/, 'variable'],                                // ${interpolation}
                [/"/, 'string', '@string'],
                [/\b(true|false|yes|no|null|none)\b/, 'constant'],
                [/\b\d+(\.\d+){0,3}\b/, 'number'],
                [/->|-->|=>/, 'operator'],
                [/[\[\]{},]/, 'delimiter'],
            ],
            string: [[/\$\{[^}]*\}/, 'variable'], [/[^\\"$]+/, 'string'], [/\\./, 'string.escape'], [/\$/, 'string'], [/"/, 'string', '@pop']],
        },
    };
    langs.register({ id: 'hk', extensions: ['.hk'], aliases: ['HK', 'hk'] });
    langs.setLanguageConfiguration('hk', { ...common, comments: { lineComment: '!' } });
    langs.setMonarchTokensProvider('hk', hkTokenizer as any);

    langs.register({ id: 'blue', extensions: ['.blue'], aliases: ['Blue', 'blue'] });
    langs.setLanguageConfiguration('blue', { ...common, comments: { lineComment: '!' } });
    langs.setMonarchTokensProvider('blue', hkTokenizer as any);

    // ── Hacker (.hacker): `[ … ]` blocks, v1 / v2 / v3 templates ────────
    langs.register({ id: 'hacker', extensions: ['.hacker'], aliases: ['Hacker', 'hacker'] });
    langs.setLanguageConfiguration('hacker', { ...common, comments: { lineComment: '#' } });
    langs.setMonarchTokensProvider('hacker', {
        defaultToken: '',
        tokenizer: {
            root: [
                [/^\s*(#|!|;;|\/\/).*$/, 'comment'],
                [/^\s*\[\s*[^\]\n]*\]?/, 'type'],                           // [ block ]
                [/^(\s*)([A-Za-z_][\w.\-]*)(\s*)(=>|=|:)/, ['white', 'variable.name', 'white', 'operator']],
                [/\$\{[^}]*\}/, 'variable'],
                [/"/, 'string', '@string'],
                [/\b(true|false|yes|no|null)\b/, 'constant'],
                [/\b\d+(\.\d+)*\b/, 'number'],
                [/->|-->|=>/, 'operator'],
                [/[\[\]{},]/, 'delimiter'],
            ],
            string: [[/\$\{[^}]*\}/, 'variable'], [/[^\\"$]+/, 'string'], [/\\./, 'string.escape'], [/\$/, 'string'], [/"/, 'string', '@pop']],
        },
    });
}
