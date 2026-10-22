import { StreamLanguage, type StreamParser, type StringStream } from '@codemirror/language';

export const HACKER_LANG_IDS = ['hsharp', 'hackerlang', 'hackerscript', 'hk', 'hacker', 'blue'] as const;

/** extension (no dot) → language id. Merged into languageMap.ts. */
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

// ── CodeMirror 6 stream parsers (ports of the former Monaco Monarch grammars) ──
// Token names are the standard StreamLanguage ones (keyword, string, comment …).

const set = (...w: string[]) => new Set(w);

interface St { mode: 'root' | 'string' | 'block' | 'cblock' | 'interp'; }

/** Eats a double-quoted string body. Returns true when the closing quote was reached. */
function eatString(stream: StringStream, interpolation: RegExp | null): boolean {
    let ch: string | void;
    while ((ch = stream.next()) != null) {
        if (ch === '\\') { stream.next(); continue; }
        if (ch === '"') return true;
        if (interpolation && interpolation.test(ch) && stream.peek() === '{') return false; // caller handles
    }
    return false;
}

function words(parserSets: { keywords: Set<string>; types?: Set<string>; constants?: Set<string>; modes?: Set<string> }, w: string, nextIsCall: boolean) {
    if (parserSets.keywords.has(w)) return 'keyword';
    if (parserSets.modes?.has(w)) return 'keyword';
    if (parserSets.types?.has(w)) return 'typeName';
    if (parserSets.constants?.has(w)) return 'atom';
    if (nextIsCall) return 'variableName.function';
    return 'variableName';
}

// ── H# ───────────────────────────────────────────────────────────────────
const HSHARP = {
    keywords: set('fn', 'func', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait', 'if', 'else', 'elif', 'match', 'while', 'for', 'in', 'loop', 'break', 'continue', 'return', 'use', 'import', 'from', 'as', 'pub', 'mod', 'type', 'unsafe', 'async', 'await', 'defer', 'extern', 'static', 'self', 'Self'),
    types: set('int', 'i8', 'i16', 'i32', 'i64', 'u8', 'u16', 'u32', 'u64', 'f32', 'f64', 'bool', 'char', 'str', 'string', 'void', 'usize', 'isize', 'Vec', 'Option', 'Result', 'Box'),
    constants: set('true', 'false', 'null', 'none', 'None', 'Some', 'Ok', 'Err'),
};
export const hsharpParser: StreamParser<St> = {
    name: 'hsharp',
    startState: () => ({ mode: 'root' }),
    languageData: { commentTokens: { line: ';;', block: { open: '/*', close: '*/' } }, closeBrackets: { brackets: ['(', '[', '{', '"'] } },
    token(stream, state) {
        if (state.mode === 'string') { state.mode = eatString(stream, null) ? 'root' : 'string'; return 'string'; }
        if (state.mode === 'block') {
            while (!stream.eol()) { if (stream.match('*/')) { state.mode = 'root'; break; } stream.next(); }
            return 'comment';
        }
        if (stream.eatSpace()) return null;
        if (stream.match(';;')) { stream.skipToEnd(); return 'comment'; }
        if (stream.match('/*')) { state.mode = 'block'; return 'comment'; }
        if (stream.match(/^#\[[^\]]*\]/)) return 'meta';
        if (stream.eat('"')) { state.mode = eatString(stream, null) ? 'root' : 'string'; return 'string'; }
        if (stream.match(/^'(?:[^'\\]|\\.)'/)) return 'string';
        if (stream.match(/^0x[0-9a-fA-F_]+/)) return 'number';
        if (stream.match(/^\d[\d_]*(\.\d+)?/)) return 'number';
        if (stream.match(/^[A-Za-z_][A-Za-z0-9_]*/)) {
            const w = stream.current();
            return words(HSHARP, w, !!stream.match(/^\s*\(/, false));
        }
        if (stream.match(/^(->|=>|::|\.\.=?|&&|\|\||[+\-*/%&|^!<>=]=?)/)) return 'operator';
        stream.next();
        return null;
    },
};

// ── Hacker Lang (.hl) ────────────────────────────────────────────────────
const HACKERLANG_KW = set('done', 'using', 'if', 'else', 'elif', 'for', 'while', 'in', 'do', 'then', 'end', 'fn', 'return', 'break', 'continue', 'import', 'spawn', 'wait');
export const hackerLangParser: StreamParser<St> = {
    name: 'hackerlang',
    startState: () => ({ mode: 'root' }),
    languageData: { commentTokens: { line: ';;' }, closeBrackets: { brackets: ['(', '[', '{', '"'] } },
    token(stream, state) {
        if (state.mode === 'string') {
            if (stream.match(/^\$\([^)]*\)/) || stream.match(/^@[A-Za-z_]\w*/)) return 'variableName';
            while (!stream.eol()) {
                const c = stream.peek();
                if (c === '\\') { stream.next(); stream.next(); return 'string'; }
                if (c === '"') { stream.next(); state.mode = 'root'; return 'string'; }
                if (c === '$' || c === '@') break;
                stream.next();
            }
            if (stream.current() === '') stream.next();
            return 'string';
        }
        if (stream.sol() && stream.match(/^#!.*$/)) return 'comment';
        if (stream.eatSpace()) return null;
        if (stream.match(/^(;;;|\/\/\/).*/)) return 'comment';
        if (stream.match(/^(;;|\/\/).*/)) return 'comment';
        if (stream.match(/^~>/)) return 'keyword';
        if (stream.match(/^::[A-Za-z_]\w*/)) return 'variableName.function';
        if (stream.match(/^\$\([^)]*\)/)) return 'variableName.special';
        if (stream.match(/^[@$][A-Za-z_]\w*/)) return 'variableName';
        if (stream.match(/^%\s*[A-Za-z_]\w*/)) return 'variableName.definition';
        if (stream.match(/^(\?~|:\*\*?|\*>|\*--|\/>|<<|>>|=>|\|>|\|\||&&|-->|->|--|~>|\?)/)) return 'operator';
        if (stream.eat('"')) { state.mode = 'string'; return 'string'; }
        if (stream.match(/^'[^']*'/)) return 'string';
        if (stream.match(/^\d+(\.\d+)?/)) return 'number';
        if (stream.match(/^[A-Za-z_][\w-]*/)) return HACKERLANG_KW.has(stream.current()) ? 'keyword' : 'variableName';
        if (stream.match(/^[|>&]/)) return 'operator';
        stream.next();
        return null;
    },
};

// ── HackerScript (.hcs) ──────────────────────────────────────────────────
const HCS = {
    keywords: set('fun', 'let', 'mut', 'const', 'struct', 'enum', 'impl', 'trait', 'if', 'else', 'elif', 'match', 'while', 'for', 'in', 'loop', 'break', 'continue', 'return', 'end', 'get', 'use', 'import', 'from', 'as', 'pub', 'defer', 'async', 'await'),
    modes: set('direct', 'manual', 'native', 'region', 'extern', 'static', 'dynamic', 'unsafe'),
    types: set('int', 'float', 'bool', 'str', 'string', 'char', 'void', 'list', 'map', 'any', 'i32', 'i64', 'u32', 'u64', 'f32', 'f64', 'usize'),
    constants: set('true', 'false', 'null', 'none'),
};
export const hackerScriptParser: StreamParser<St> = {
    name: 'hackerscript',
    startState: () => ({ mode: 'root' }),
    languageData: { commentTokens: { line: '//', block: { open: '!>', close: '<!' } }, closeBrackets: { brackets: ['(', '[', '{', '"'] } },
    token(stream, state) {
        if (state.mode === 'string') { state.mode = eatString(stream, null) ? 'root' : 'string'; return 'string'; }
        if (state.mode === 'block') {
            while (!stream.eol()) { if (stream.match('<!')) { state.mode = 'root'; break; } stream.next(); }
            return 'comment';
        }
        if (state.mode === 'cblock') {
            while (!stream.eol()) { if (stream.match('*/')) { state.mode = 'root'; break; } stream.next(); }
            return 'comment';
        }
        if (stream.eatSpace()) return null;
        if (stream.match(/^(!!|\/\/).*/)) return 'comment';
        if (stream.match('!>')) { state.mode = 'block'; return 'comment'; }
        if (stream.match('/*')) { state.mode = 'cblock'; return 'comment'; }
        if (stream.eat('"')) { state.mode = eatString(stream, null) ? 'root' : 'string'; return 'string'; }
        if (stream.match(/^'(?:[^'\\]|\\.)'/)) return 'string';
        if (stream.match(/^<[A-Za-z_][\w:./-]*>/)) return 'string.special';
        if (stream.match(/^0x[0-9a-fA-F_]+/)) return 'number';
        if (stream.match(/^\d[\d_]*(\.\d+)?/)) return 'number';
        if (stream.match(/^[A-Za-z_]\w*/)) return words(HCS, stream.current(), !!stream.match(/^\s*\(/, false));
        if (stream.match(/^(->|=>|::|\.\.=?|&&|\|\||[+\-*/%&|^!<>=]=?)/)) return 'operator';
        stream.next();
        return null;
    },
};

// ── HK (.hk) / Blue (.blue) — same family ───────────────────────────────
function hkLike(name: string, lineComment: string): StreamParser<St> {
    return {
        name,
        startState: () => ({ mode: 'root' }),
        languageData: { commentTokens: { line: lineComment }, closeBrackets: { brackets: ['(', '[', '{', '"'] } },
        token(stream, state) {
            if (state.mode === 'string') {
                if (stream.match(/^\$\{[^}]*\}/)) return 'variableName';
                while (!stream.eol()) {
                    const c = stream.peek();
                    if (c === '\\') { stream.next(); stream.next(); return 'string'; }
                    if (c === '"') { stream.next(); state.mode = 'root'; return 'string'; }
                    if (c === '$') break;
                    stream.next();
                }
                if (stream.current() === '') stream.next();
                return 'string';
            }
            if (stream.sol()) {
                if (stream.match(/^\s*(!|#|;).*/)) return 'comment';
                if (stream.match(/^\s*\[[^\]]*\]/)) return 'typeName';
                if (stream.match(/^\s*[A-Za-z_][\w.\-]*(?=\s*(=>|=))/)) return 'propertyName';
                if (stream.match(/^\s*(->|-->)(?=\s*[A-Za-z_])/)) return 'operator';
            }
            if (stream.eatSpace()) return null;
            if (stream.match(/^\$\{[^}]*\}/)) return 'variableName';
            if (stream.eat('"')) { state.mode = 'string'; return 'string'; }
            if (stream.match(/^(true|false|yes|no|null|none)\b/)) return 'atom';
            if (stream.match(/^\d+(\.\d+){0,3}/)) return 'number';
            if (stream.match(/^(-->|->|=>|=)/)) return 'operator';
            if (stream.match(/^[A-Za-z_][\w.\-]*/)) return 'propertyName';
            stream.next();
            return null;
        },
    };
}
export const hkParser = hkLike('hk', '!');
export const blueParser = hkLike('blue', '!');

// ── Hacker (.hacker) ─────────────────────────────────────────────────────
export const hackerParser: StreamParser<St> = {
    name: 'hacker',
    startState: () => ({ mode: 'root' }),
    languageData: { commentTokens: { line: '#' }, closeBrackets: { brackets: ['(', '[', '{', '"'] } },
    token(stream, state) {
        if (state.mode === 'string') {
            if (stream.match(/^\$\{[^}]*\}/)) return 'variableName';
            while (!stream.eol()) {
                const c = stream.peek();
                if (c === '\\') { stream.next(); stream.next(); return 'string'; }
                if (c === '"') { stream.next(); state.mode = 'root'; return 'string'; }
                if (c === '$') break;
                stream.next();
            }
            if (stream.current() === '') stream.next();
            return 'string';
        }
        if (stream.sol()) {
            if (stream.match(/^\s*(#|!|;;|\/\/).*/)) return 'comment';
            if (stream.match(/^\s*\[\s*[^\]\n]*\]?/)) return 'typeName';
            if (stream.match(/^\s*[A-Za-z_][\w.\-]*(?=\s*(=>|=|:))/)) return 'propertyName';
        }
        if (stream.eatSpace()) return null;
        if (stream.match(/^\$\{[^}]*\}/)) return 'variableName';
        if (stream.eat('"')) { state.mode = 'string'; return 'string'; }
        if (stream.match(/^(true|false|yes|no|null)\b/)) return 'atom';
        if (stream.match(/^\d+(\.\d+)*/)) return 'number';
        if (stream.match(/^(-->|->|=>|=|:)/)) return 'operator';
        stream.next();
        return null;
    },
};

const PARSERS: Record<string, StreamParser<St>> = {
    hsharp: hsharpParser, hackerlang: hackerLangParser, hackerscript: hackerScriptParser,
    hk: hkParser, blue: blueParser, hacker: hackerParser,
};

/** CodeMirror language for one of the HackerOS-family ids (or null). */
export function hackerLanguage(id: string) {
    const p = PARSERS[id];
    return p ? StreamLanguage.define(p) : null;
}
