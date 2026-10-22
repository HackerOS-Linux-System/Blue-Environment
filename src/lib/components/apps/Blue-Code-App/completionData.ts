type Monaco = any;

// ── Keyword banks ──────────────────────────────────────────────────────────

const KW: Record<string, string[]> = {
    javascript: [
        'const','let','var','function','async','await','return','import','export',
        'default','class','extends','new','this','super','typeof','instanceof',
        'if','else','for','while','do','switch','case','break','continue','try',
        'catch','finally','throw','null','undefined','true','false','void','in',
        'of','yield','from','as','static','get','set','delete','Promise',
        'console','document','window','Array','Object','String','Number','Math',
        'JSON','fetch','localStorage','sessionStorage','setTimeout','clearTimeout',
        'setInterval','clearInterval','Map','Set','WeakMap','WeakSet','Symbol',
        'Proxy','Reflect','Intl','Error','TypeError','RangeError','SyntaxError',
        'module','require','process','Buffer',
    ],
    typescript: [
        'const','let','var','function','async','await','return','import','export',
        'default','class','extends','implements','new','this','super','typeof',
        'instanceof','if','else','for','while','switch','case','break','continue',
        'try','catch','finally','throw','null','undefined','true','false','void',
        'in','of','yield','from','as','type','interface','enum','namespace',
        'declare','abstract','readonly','private','protected','public','override',
        'never','unknown','any','string','number','boolean','object','bigint',
        'symbol','keyof','infer','conditional','mapped','template','utility',
        'Partial','Required','Readonly','Record','Pick','Omit','Exclude','Extract',
        'NonNullable','ReturnType','Parameters','InstanceType','Awaited',
        'Promise','Array','Tuple','Union','Intersection','Generic',
    ],
    python: [
        'def','class','return','import','from','as','if','elif','else','for',
        'while','break','continue','pass','try','except','finally','raise','with',
        'lambda','yield','async','await','True','False','None','and','or','not',
        'in','is','global','nonlocal','del','assert','print','len','range','list',
        'dict','set','tuple','str','int','float','bool','type','isinstance','issubclass',
        'super','self','cls','property','staticmethod','classmethod','open','os',
        'sys','re','json','math','random','datetime','pathlib','subprocess',
        'threading','asyncio','typing','dataclasses','functools','itertools',
        'collections','contextlib','abc','enum','copy','io','time','uuid',
    ],
    rust: [
        'fn','let','mut','pub','use','mod','struct','enum','impl','trait','where',
        'return','if','else','for','while','loop','match','break','continue',
        'move','async','await','dyn','Box','Arc','Rc','RefCell','Cell','Mutex',
        'RwLock','Vec','HashMap','HashSet','BTreeMap','BTreeSet','Option','Result',
        'Some','None','Ok','Err','String','str','i8','i16','i32','i64','i128',
        'isize','u8','u16','u32','u64','u128','usize','f32','f64','bool','char',
        'true','false','Self','self','super','crate','extern','const','static',
        'type','unsafe','ref','as','in','derive','allow','warn','deny','cfg',
        'feature','test','println','eprintln','format','vec','panic','todo',
        'unimplemented','unreachable','assert','assert_eq','assert_ne','dbg',
    ],
    go: [
        'func','var','const','type','struct','interface','map','slice','chan',
        'go','defer','return','if','else','for','range','switch','case','break',
        'continue','fallthrough','select','default','import','package','make',
        'new','append','len','cap','copy','delete','close','panic','recover',
        'error','nil','true','false','int','int8','int16','int32','int64',
        'uint','uint8','uint16','uint32','uint64','uintptr','float32','float64',
        'complex64','complex128','string','bool','byte','rune','any',
        'fmt','os','io','net','http','json','context','sync','time','strings',
        'strconv','bufio','bytes','errors','log','math','sort','path','regexp',
    ],
    cpp: [
        'int','float','double','char','bool','void','auto','const','constexpr',
        'static','inline','extern','register','volatile','mutable','explicit',
        'virtual','override','final','abstract','public','protected','private',
        'class','struct','union','enum','namespace','using','typedef','template',
        'typename','concept','requires','if','else','for','while','do','switch',
        'case','break','continue','return','goto','try','catch','throw','new',
        'delete','nullptr','true','false','this','sizeof','alignof','typeid',
        'noexcept','decltype','static_cast','dynamic_cast','reinterpret_cast',
        'const_cast','std','vector','string','map','set','unordered_map','pair',
        'unique_ptr','shared_ptr','weak_ptr','make_unique','make_shared',
        'cout','cin','endl','include','define','pragma','ifdef','ifndef',
    ],
    css: [
        'display','position','flex','grid','width','height','margin','padding',
        'color','background','border','border-radius','font-size','font-weight',
        'font-family','line-height','text-align','text-decoration','text-transform',
        'opacity','transform','transition','animation','overflow','z-index',
        'cursor','pointer-events','box-shadow','outline','visibility','content',
        'top','right','bottom','left','right','float','clear','vertical-align',
        'white-space','word-break','word-wrap','max-width','min-width','max-height',
        'min-height','object-fit','object-position','aspect-ratio','gap','column-gap',
        'row-gap','align-items','justify-content','align-self','justify-self',
        'flex-direction','flex-wrap','flex-grow','flex-shrink','flex-basis',
        'grid-template-columns','grid-template-rows','grid-column','grid-row',
        'absolute','relative','fixed','sticky','block','inline','inline-block',
        'none','flex','grid','inherit','initial','unset','var','calc','min','max',
        'clamp','rgba','rgb','hsl','hsla','px','em','rem','vh','vw','%',
    ],
    html: [
        'html','head','body','div','span','p','h1','h2','h3','h4','h5','h6',
        'a','img','ul','ol','li','table','tr','td','th','thead','tbody','tfoot',
        'form','input','button','select','option','textarea','label','fieldset',
        'section','article','aside','header','footer','nav','main','figure',
        'figcaption','blockquote','pre','code','em','strong','b','i','u','s',
        'sup','sub','hr','br','iframe','video','audio','source','canvas','svg',
        'path','circle','rect','class','id','style','href','src','alt','type',
        'value','placeholder','name','action','method','for','target','rel',
        'data-','aria-','role','tabindex','contenteditable','draggable',
    ],
    json: [
        'true','false','null',
    ],
    markdown: [
        '# Heading 1','## Heading 2','### Heading 3',
        '**bold**','*italic*','`code`','```','---','> Quote',
        '- List item','1. Ordered','[text](url)','![alt](url)',
        '| Column |','| --- |',
    ],
};

// ── Snippets ───────────────────────────────────────────────────────────────

interface Snippet {
    label: string;
    detail: string;
    insertText: string;
    documentation?: string;
}

const SNIPPETS: Record<string, Snippet[]> = {
    javascript: [
        { label: 'fn', detail: 'Arrow function', insertText: 'const ${1:name} = (${2:params}) => {\n\t${3:// body}\n};' },
        { label: 'async-fn', detail: 'Async arrow function', insertText: 'const ${1:name} = async (${2:params}) => {\n\tconst ${3:result} = await ${4:promise};\n\treturn ${3:result};\n};' },
        { label: 'try', detail: 'Try/catch block', insertText: 'try {\n\t${1:// code}\n} catch (${2:error}) {\n\tconsole.error(${2:error});\n}' },
        { label: 'fetch', detail: 'Fetch with error handling', insertText: 'const response = await fetch(\'${1:url}\');\nif (!response.ok) throw new Error(`HTTP \${response.status}`);\nconst data = await response.json();' },
        { label: 'class', detail: 'ES6 class', insertText: 'class ${1:Name} extends ${2:Base} {\n\tconstructor(${3:params}) {\n\t\tsuper();\n\t\t${4:// init}\n\t}\n\n\t${5:method}() {\n\t\t${6:// body}\n\t}\n}' },
        { label: 'switch', detail: 'Switch statement', insertText: 'switch (${1:expr}) {\n\tcase ${2:value}:\n\t\t${3:// code}\n\t\tbreak;\n\tdefault:\n\t\t${4:// default}\n}' },
        { label: 'forin', detail: 'For...of loop', insertText: 'for (const ${1:item} of ${2:array}) {\n\t${3:// body}\n}' },
        { label: 'promise', detail: 'New Promise', insertText: 'new Promise((resolve, reject) => {\n\t${1:// async work}\n\tresolve(${2:value});\n})' },
    ],
    typescript: [
        { label: 'interface', detail: 'TypeScript interface', insertText: 'interface ${1:Name} {\n\t${2:property}: ${3:type};\n}' },
        { label: 'type', detail: 'Type alias', insertText: 'type ${1:Name} = ${2:definition};' },
        { label: 'generic', detail: 'Generic function', insertText: 'function ${1:name}<${2:T}>(${3:arg}: ${2:T}): ${4:ReturnType} {\n\t${5:// body}\n}' },
        { label: 'enum', detail: 'Enum', insertText: 'enum ${1:Name} {\n\t${2:Value1} = \'${3:value1}\',\n\t${4:Value2} = \'${5:value2}\',\n}' },
        { label: 'react-fc', detail: 'React functional component', insertText: 'import React from \'react\';\n\ninterface ${1:Name}Props {\n\t${2:prop}: ${3:type};\n}\n\nexport const ${1:Name}: React.FC<${1:Name}Props> = ({ ${2:prop} }) => {\n\treturn (\n\t\t<div>\n\t\t\t${4:// content}\n\t\t</div>\n\t);\n};\n\nexport default ${1:Name};' },
        { label: 'useState', detail: 'React useState hook', insertText: 'const [${1:state}, set${1/(.+)/\\u$1/}] = useState<${2:type}>(${3:initial});' },
        { label: 'useEffect', detail: 'React useEffect hook', insertText: 'useEffect(() => {\n\t${1:// effect}\n\treturn () => {\n\t\t${2:// cleanup}\n\t};\n}, [${3:deps}]);' },
        { label: 'useCallback', detail: 'React useCallback hook', insertText: 'const ${1:fn} = useCallback(${2:(args)} => {\n\t${3:// body}\n}, [${4:deps}]);' },
    ],
    python: [
        { label: 'def', detail: 'Function definition', insertText: 'def ${1:name}(${2:args}):\n\t"""${3:docstring}"""\n\t${4:pass}' },
        { label: 'class', detail: 'Class definition', insertText: 'class ${1:Name}:\n\t"""${2:docstring}"""\n\n\tdef __init__(self${3:, args}):\n\t\t${4:pass}' },
        { label: 'dataclass', detail: 'Dataclass', insertText: 'from dataclasses import dataclass, field\n\n@dataclass\nclass ${1:Name}:\n\t${2:field}: ${3:type} = ${4:default}' },
        { label: 'try', detail: 'Try/except', insertText: 'try:\n\t${1:pass}\nexcept ${2:Exception} as ${3:e}:\n\t${4:raise}' },
        { label: 'ctx', detail: 'Context manager', insertText: 'with ${1:open}(${2:path}) as ${3:f}:\n\t${4:pass}' },
        { label: 'async-def', detail: 'Async function', insertText: 'async def ${1:name}(${2:args}) -> ${3:None}:\n\t${4:pass}' },
        { label: 'list-comp', detail: 'List comprehension', insertText: '[${1:expr} for ${2:item} in ${3:iterable}${4: if ${5:condition}}]' },
        { label: 'dict-comp', detail: 'Dict comprehension', insertText: '{${1:key}: ${2:value} for ${3:item} in ${4:iterable}}' },
    ],
    rust: [
        { label: 'fn', detail: 'Function', insertText: 'fn ${1:name}(${2:args}) -> ${3:ReturnType} {\n\t${4:todo!()}\n}' },
        { label: 'impl', detail: 'Impl block', insertText: 'impl ${1:Type} {\n\tpub fn ${2:new}(${3:args}) -> Self {\n\t\tSelf {\n\t\t\t${4:field}: ${5:value},\n\t\t}\n\t}\n}' },
        { label: 'struct', detail: 'Struct definition', insertText: '#[derive(Debug, Clone)]\npub struct ${1:Name} {\n\tpub ${2:field}: ${3:Type},\n}' },
        { label: 'enum', detail: 'Enum definition', insertText: '#[derive(Debug, Clone)]\npub enum ${1:Name} {\n\t${2:Variant1},\n\t${3:Variant2}(${4:Type}),\n}' },
        { label: 'match', detail: 'Match expression', insertText: 'match ${1:expr} {\n\t${2:Some(v)} => ${3:v},\n\t${4:None} => ${5:default},\n}' },
        { label: 'result', detail: 'Result type', insertText: 'Result<${1:Ok}, ${2:Err}>' },
        { label: 'option', detail: 'Option type', insertText: 'Option<${1:T}>' },
        { label: 'vec', detail: 'Vec with items', insertText: 'vec![${1:items}]' },
        { label: 'trait', detail: 'Trait definition', insertText: 'pub trait ${1:Name} {\n\tfn ${2:method}(&self) -> ${3:ReturnType};\n}' },
    ],
    go: [
        { label: 'func', detail: 'Function', insertText: 'func ${1:name}(${2:params}) ${3:error} {\n\t${4:// body}\n\treturn nil\n}' },
        { label: 'struct', detail: 'Struct type', insertText: 'type ${1:Name} struct {\n\t${2:Field} ${3:Type}\n}' },
        { label: 'interface', detail: 'Interface', insertText: 'type ${1:Name} interface {\n\t${2:Method}(${3:params}) ${4:ReturnType}\n}' },
        { label: 'if-err', detail: 'Error check', insertText: 'if err != nil {\n\treturn ${1:nil,} err\n}' },
        { label: 'goroutine', detail: 'Goroutine', insertText: 'go func() {\n\t${1:// code}\n}()' },
        { label: 'channel', detail: 'Channel', insertText: 'ch := make(chan ${1:Type}, ${2:buffer})' },
        { label: 'defer', detail: 'Defer', insertText: 'defer ${1:func}()' },
        { label: 'for-range', detail: 'For range', insertText: 'for ${1:i}, ${2:v} := range ${3:slice} {\n\t${4:_ = v}\n}' },
    ],
};

// ── Word provider (scan current file) ─────────────────────────────────────

function getWordCompletions(model: import('monaco-editor').editor.ITextModel, position: import('monaco-editor').Position, MonacoLib: typeof import('monaco-editor')) {
    const word = model.getWordUntilPosition(position);
    const range = {
        startLineNumber: position.lineNumber,
        endLineNumber:   position.lineNumber,
        startColumn:     word.startColumn,
        endColumn:       word.endColumn,
    };

    const text    = model.getValue();
    const wordSet = new Set<string>();
    const current = word.word.toLowerCase();

    for (const match of text.matchAll(/\b([a-zA-Z_][a-zA-Z0-9_]{2,})\b/g)) {
        const w = match[1];
        if (w.toLowerCase() !== current) wordSet.add(w);
    }

    return [...wordSet].slice(0, 40).map(w => ({
        label:      w,
        kind:       MonacoLib.languages.CompletionItemKind.Text,
        insertText: w,
        range,
        detail:     'Word in document',
        sortText:   'z' + w,
    }));
}

// ── Registration ───────────────────────────────────────────────────────────

export function registerCompletionProviders(monaco: Monaco) {
    const allLangs = ['javascript','typescript','python','rust','go','cpp','c','css','html','json','markdown'];

    for (const lang of allLangs) {
        monaco.languages.registerCompletionItemProvider(lang, {
            triggerCharacters: ['.', ' ', '(', '{', '<', '"', "'", '/'],

            provideCompletionItems(model: import('monaco-editor').editor.ITextModel, position: import('monaco-editor').Position) {
                const word = model.getWordUntilPosition(position);
                const range = {
                    startLineNumber: position.lineNumber,
                    endLineNumber:   position.lineNumber,
                    startColumn:     word.startColumn,
                    endColumn:       word.endColumn,
                };

                const suggestions: any[] = [];

                // 1) Keywords
                const kwList = KW[lang] ?? [];
                for (const kw of kwList) {
                    suggestions.push({
                        label:      kw,
                        kind:       monaco.languages.CompletionItemKind.Keyword,
                        insertText: kw,
                        range,
                        detail:     `${lang} keyword`,
                        sortText:   'a' + kw,
                    });
                }

                // 2) Snippets
                const snips = SNIPPETS[lang] ?? [];
                for (const snip of snips) {
                    suggestions.push({
                        label:            snip.label,
                        kind:             monaco.languages.CompletionItemKind.Snippet,
                        insertText:       snip.insertText,
                        insertTextRules:  monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
                        range,
                        detail:           snip.detail,
                        documentation:    snip.documentation,
                        sortText:         'b' + snip.label,
                    });
                }

                // 3) Words in current document
                const wordItems = getWordCompletions(model, position, monaco);
                suggestions.push(...wordItems);

                return { suggestions };
            },
        });
    }

    // ── Hover provider — show type info & docs ─────────────────────────────
    for (const lang of ['javascript','typescript','python','rust','go']) {
        monaco.languages.registerHoverProvider(lang, {
            provideHover(model: import('monaco-editor').editor.ITextModel, position: import('monaco-editor').Position) {
                const word = model.getWordAtPosition(position);
                if (!word) return null;

                const w = word.word;
                const kwList = KW[lang] ?? [];
                if (!kwList.includes(w)) return null;

                const docs: Record<string, string> = {
                    'async':  '**async** — Marks a function as asynchronous; it returns a Promise.',
                    'await':  '**await** — Pauses execution until the Promise resolves.',
                    'const':  '**const** — Block-scoped constant; cannot be reassigned.',
                    'let':    '**let** — Block-scoped variable.',
                    'fn':     '**fn** — Rust function keyword.',
                    'match':  '**match** — Rust pattern-matching expression (exhaustive).',
                    'mut':    '**mut** — Rust keyword: marks a variable or reference as mutable.',
                    'def':    '**def** — Python function definition keyword.',
                    'yield':  '**yield** — Produces a value from a generator function.',
                    'defer':  '**defer** — Go: execute a statement when the surrounding function returns.',
                    'goroutine': '**go** — Go: launches a goroutine (lightweight concurrent function).',
                };

                const content = docs[w];
                if (!content) return null;
                return { contents: [{ value: content }] };
            },
        });
    }

    // ── Signature help ─────────────────────────────────────────────────────
    monaco.languages.registerSignatureHelpProvider('javascript', {
        signatureHelpTriggerCharacters: ['(', ','],
        provideSignatureHelp(model: import('monaco-editor').editor.ITextModel, position: import('monaco-editor').Position) {
            const lineText = model.getLineContent(position.lineNumber);
            const prefix   = lineText.substring(0, position.column - 1);

            const APIs: Record<string, { sig: string; params: string[] }> = {
                'setTimeout':   { sig: 'setTimeout(fn, delay)', params: ['fn: () => void', 'delay: number'] },
                'setInterval':  { sig: 'setInterval(fn, delay)', params: ['fn: () => void', 'delay: number'] },
                'fetch':        { sig: 'fetch(url, init?)', params: ['url: string | URL', 'init?: RequestInit'] },
                'console.log':  { sig: 'console.log(...data)', params: ['...data: any[]'] },
                'console.error':{ sig: 'console.error(...data)', params: ['...data: any[]'] },
                'Array.from':   { sig: 'Array.from(arrayLike, mapFn?)', params: ['arrayLike: ArrayLike<T>', 'mapFn?: (value: T, index: number) => U'] },
                'Object.keys':  { sig: 'Object.keys(obj)', params: ['obj: object'] },
                'Object.values':{ sig: 'Object.values(obj)', params: ['obj: object'] },
                'Object.assign':{ sig: 'Object.assign(target, ...sources)', params: ['target: object', '...sources: object[]'] },
                'JSON.parse':   { sig: 'JSON.parse(text, reviver?)', params: ['text: string', 'reviver?: (key, value) => any'] },
                'JSON.stringify':{ sig: 'JSON.stringify(value, replacer?, space?)', params: ['value: any', 'replacer?: any', 'space?: string | number'] },
                'Math.max':     { sig: 'Math.max(...values)', params: ['...values: number[]'] },
                'Math.min':     { sig: 'Math.min(...values)', params: ['...values: number[]'] },
                'Math.floor':   { sig: 'Math.floor(x)', params: ['x: number'] },
                'Math.ceil':    { sig: 'Math.ceil(x)', params: ['x: number'] },
                'Math.round':   { sig: 'Math.round(x)', params: ['x: number'] },
                'Math.random':  { sig: 'Math.random()', params: [] },
                'Math.abs':     { sig: 'Math.abs(x)', params: ['x: number'] },
                'parseInt':     { sig: 'parseInt(string, radix?)', params: ['string: string', 'radix?: number (2-36)'] },
                'parseFloat':   { sig: 'parseFloat(string)', params: ['string: string'] },
            };

            for (const [fn, info] of Object.entries(APIs)) {
                if (prefix.includes(fn)) {
                    const commasBefore = (prefix.split('(').pop() ?? '').split(',').length - 1;
                    return {
                        value: {
                            signatures: [{
                                label: info.sig,
                                parameters: info.params.map(p => ({ label: p })),
                                activeParameter: commasBefore,
                            }],
                            activeSignature: 0,
                            activeParameter: commasBefore,
                        },
                        dispose() {},
                    };
                }
            }
            return null;
        },
    });
}

// ── Custom Monaco themes ───────────────────────────────────────────────────

export function registerEditorThemes(monaco: Monaco) {
    monaco.editor.defineTheme('blue-dark', {
        base: 'vs-dark',
        inherit: true,
        rules: [
            { token: 'keyword',        foreground: '60a5fa', fontStyle: 'bold' },
            { token: 'keyword.flow',   foreground: 'a78bfa', fontStyle: 'bold' },
            { token: 'string',         foreground: '34d399' },
            { token: 'string.escape',  foreground: '6ee7b7' },
            { token: 'comment',        foreground: '475569', fontStyle: 'italic' },
            { token: 'number',         foreground: 'fb923c' },
            { token: 'type',           foreground: '38bdf8' },
            { token: 'class',          foreground: 'fbbf24' },
            { token: 'function',       foreground: '93c5fd' },
            { token: 'variable',       foreground: 'e2e8f0' },
            { token: 'delimiter',      foreground: '64748b' },
            { token: 'tag',            foreground: '60a5fa' },
            { token: 'attribute.name', foreground: '93c5fd' },
            { token: 'attribute.value',foreground: '34d399' },
        ],
        colors: {
            'editor.background':              '#0f172a',
            'editor.foreground':              '#e2e8f0',
            'editor.lineHighlightBackground': '#1e293b',
            'editor.selectionBackground':     '#1d4ed8aa',
            'editor.inactiveSelectionBackground': '#1e3a8a55',
            'editorCursor.foreground':        '#60a5fa',
            'editorLineNumber.foreground':    '#334155',
            'editorLineNumber.activeForeground': '#60a5fa',
            'editorIndentGuide.background':   '#1e293b',
            'editorIndentGuide.activeBackground': '#334155',
            'editorSuggestWidget.background': '#0f172a',
            'editorSuggestWidget.border':     '#1e40af',
            'editorSuggestWidget.selectedBackground': '#1e3a8a',
            'editorHoverWidget.background':   '#0f172a',
            'editorHoverWidget.border':       '#1e40af',
            'editorWidget.background':        '#0f172a',
            'editorWidget.border':            '#1e40af',
            'input.background':               '#1e293b',
            'input.border':                   '#334155',
            'scrollbarSlider.background':     '#1e293b',
            'scrollbarSlider.hoverBackground':'#334155',
            'scrollbarSlider.activeBackground':'#3b82f6',
            'minimap.background':             '#0a0f1e',
        },
    });

    monaco.editor.defineTheme('blue-light', {
        base: 'vs',
        inherit: true,
        rules: [
            { token: 'keyword',  foreground: '1d4ed8', fontStyle: 'bold' },
            { token: 'string',   foreground: '059669' },
            { token: 'comment',  foreground: '94a3b8', fontStyle: 'italic' },
            { token: 'number',   foreground: 'd97706' },
            { token: 'type',     foreground: '0284c7' },
            { token: 'function', foreground: '7c3aed' },
        ],
        colors: {
            'editor.background': '#f8fafc',
            'editor.foreground': '#1e293b',
            'editor.lineHighlightBackground': '#f1f5f9',
            'editorCursor.foreground': '#1d4ed8',
            'editorLineNumber.foreground': '#94a3b8',
        },
    });

    monaco.editor.defineTheme('blue-ocean', {
        base: 'vs-dark',
        inherit: true,
        rules: [
            { token: 'keyword', foreground: '00b4d8', fontStyle: 'bold' },
            { token: 'string',  foreground: '90e0ef' },
            { token: 'comment', foreground: '4a7c9e', fontStyle: 'italic' },
            { token: 'number',  foreground: 'ffb703' },
            { token: 'type',    foreground: '48cae4' },
            { token: 'function',foreground: 'caf0f8' },
        ],
        colors: {
            'editor.background': '#023047',
            'editor.foreground': '#caf0f8',
            'editor.lineHighlightBackground': '#03405f',
            'editorCursor.foreground': '#00b4d8',
            'editorLineNumber.foreground': '#2a6080',
        },
    });
}
