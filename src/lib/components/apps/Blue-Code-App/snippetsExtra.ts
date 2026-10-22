import type { Snippet } from './completionData';

/**
 * Extra snippets for the languages the old Monaco setup did not cover, plus a
 * richer HTML/CSS/JS set. Syntax is VS Code / LSP snippet syntax
 * (`${1:placeholder}`, `$0`), which CodeMirror's `snippet()` understands.
 */
const S = (label: string, detail: string, insertText: string): Snippet => ({ label, detail, insertText });

export const HTML5_TEMPLATE =
  '<!DOCTYPE html>\n<html lang="${1:en}">\n<head>\n\t<meta charset="UTF-8">\n\t<meta name="viewport" content="width=device-width, initial-scale=1.0">\n\t<title>${2:Document}</title>\n</head>\n<body>\n\t$0\n</body>\n</html>\n';

export const EXTRA_SNIPPETS: Record<string, Snippet[]> = {
  html: [
    S('!', 'HTML5 boilerplate (like VS Code)', HTML5_TEMPLATE),
    S('html:5', 'HTML5 boilerplate', HTML5_TEMPLATE),
    S('link:css', 'Stylesheet link', '<link rel="stylesheet" href="${1:style.css}">'),
    S('script:src', 'External script', '<script src="${1:app.js}"></script>'),
    S('meta:vp', 'Viewport meta', '<meta name="viewport" content="width=device-width, initial-scale=1.0">'),
    S('a:link', 'Anchor', '<a href="${1:https://}">${2:text}</a>'),
    S('img', 'Image', '<img src="${1:path}" alt="${2:description}">'),
    S('form:post', 'Form', '<form action="${1:/submit}" method="post">\n\t$0\n</form>'),
    S('table', 'Table', '<table>\n\t<thead>\n\t\t<tr><th>${1:Header}</th></tr>\n\t</thead>\n\t<tbody>\n\t\t<tr><td>${2:Cell}</td></tr>\n\t</tbody>\n</table>'),
    S('ul>li', 'List', '<ul>\n\t<li>${1:item}</li>\n\t<li>${2:item}</li>\n</ul>'),
  ],
  css: [
    S('flex-center', 'Center with flexbox', 'display: flex;\nalign-items: center;\njustify-content: center;'),
    S('grid-cols', 'CSS grid columns', 'display: grid;\ngrid-template-columns: repeat(${1:3}, 1fr);\ngap: ${2:1rem};'),
    S('media', 'Media query', '@media (max-width: ${1:768px}) {\n\t$0\n}'),
    S('var', 'CSS variable', 'var(--${1:name})'),
    S('keyframes', 'Keyframes', '@keyframes ${1:name} {\n\tfrom { $2 }\n\tto { $3 }\n}'),
    S('dark-scheme', 'prefers-color-scheme: dark', '@media (prefers-color-scheme: dark) {\n\t$0\n}'),
  ],
  javascript: [
    S('log', 'console.log', 'console.log(${1:value});'),
    S('arrow', 'Arrow function', 'const ${1:name} = (${2:args}) => {\n\t$0\n};'),
    S('async-fn', 'Async function', 'async function ${1:name}(${2:args}) {\n\t$0\n}'),
    S('try', 'try / catch', 'try {\n\t$1\n} catch (${2:err}) {\n\t$0\n}'),
    S('fetch', 'fetch + json', 'const res = await fetch(${1:url});\nconst data = await res.json();'),
    S('import', 'import', "import { $2 } from '${1:module}';"),
    S('foreach', 'forEach', '${1:array}.forEach((${2:item}) => {\n\t$0\n});'),
  ],
  java: [
    S('main', 'public static void main', 'public static void main(String[] args) {\n\t$0\n}'),
    S('sout', 'System.out.println', 'System.out.println(${1:value});'),
    S('class', 'Class', 'public class ${1:Name} {\n\t$0\n}'),
    S('fori', 'for loop', 'for (int ${1:i} = 0; ${1:i} < ${2:n}; ${1:i}++) {\n\t$0\n}'),
    S('trycatch', 'try / catch', 'try {\n\t$1\n} catch (${2:Exception} e) {\n\t$0\n}'),
  ],
  kotlin: [
    S('main', 'fun main', 'fun main() {\n\t$0\n}'),
    S('fun', 'Function', 'fun ${1:name}(${2:args}): ${3:Unit} {\n\t$0\n}'),
    S('data', 'Data class', 'data class ${1:Name}(val ${2:field}: ${3:String})'),
    S('when', 'when', 'when (${1:x}) {\n\t${2:value} -> $0\n\telse -> {}\n}'),
    S('println', 'println', 'println(${1:value})'),
  ],
  swift: [
    S('func', 'Function', 'func ${1:name}(${2:arg}: ${3:Type}) -> ${4:Void} {\n\t$0\n}'),
    S('struct', 'Struct', 'struct ${1:Name} {\n\t$0\n}'),
    S('guard', 'guard let', 'guard let ${1:value} = ${2:optional} else {\n\t${3:return}\n}'),
    S('iflet', 'if let', 'if let ${1:value} = ${2:optional} {\n\t$0\n}'),
    S('print', 'print', 'print(${1:value})'),
  ],
  csharp: [
    S('main', 'Main', 'static void Main(string[] args)\n{\n\t$0\n}'),
    S('cw', 'Console.WriteLine', 'Console.WriteLine(${1:value});'),
    S('class', 'Class', 'public class ${1:Name}\n{\n\t$0\n}'),
    S('prop', 'Property', 'public ${1:string} ${2:Name} { get; set; }'),
    S('foreach', 'foreach', 'foreach (var ${1:item} in ${2:items})\n{\n\t$0\n}'),
  ],
  php: [
    S('php', 'PHP block', '<?php\n$0\n'),
    S('echo', 'echo', 'echo ${1:value};'),
    S('func', 'Function', 'function ${1:name}(${2:\\$args}) {\n\t$0\n}'),
    S('foreach', 'foreach', 'foreach (${1:\\$items} as ${2:\\$item}) {\n\t$0\n}'),
    S('class', 'Class', 'class ${1:Name}\n{\n\t$0\n}'),
  ],
  ruby: [
    S('def', 'Method', 'def ${1:name}(${2:args})\n  $0\nend'),
    S('class', 'Class', 'class ${1:Name}\n  $0\nend'),
    S('each', 'each', '${1:items}.each do |${2:item}|\n  $0\nend'),
    S('puts', 'puts', 'puts ${1:value}'),
  ],
  lua: [
    S('function', 'Function', 'function ${1:name}(${2:args})\n\t$0\nend'),
    S('for', 'for loop', 'for ${1:i} = ${2:1}, ${3:10} do\n\t$0\nend'),
    S('pairs', 'pairs loop', 'for ${1:k}, ${2:v} in pairs(${3:tbl}) do\n\t$0\nend'),
  ],
  sql: [
    S('select', 'SELECT', 'SELECT ${1:*}\nFROM ${2:table}\nWHERE ${3:condition};'),
    S('insert', 'INSERT', 'INSERT INTO ${1:table} (${2:cols})\nVALUES (${3:vals});'),
    S('update', 'UPDATE', 'UPDATE ${1:table}\nSET ${2:col} = ${3:val}\nWHERE ${4:condition};'),
    S('join', 'JOIN', 'JOIN ${1:table} ON ${2:a} = ${3:b}'),
    S('create', 'CREATE TABLE', 'CREATE TABLE ${1:name} (\n\tid INTEGER PRIMARY KEY,\n\t$0\n);'),
  ],
  shell: [
    S('shebang', 'Bash shebang', '#!/usr/bin/env bash\nset -euo pipefail\n\n$0'),
    S('if', 'if', 'if [[ ${1:cond} ]]; then\n\t$0\nfi'),
    S('for', 'for', 'for ${1:i} in ${2:list}; do\n\t$0\ndone'),
    S('func', 'function', '${1:name}() {\n\t$0\n}'),
  ],
  yaml: [
    S('compose', 'docker-compose service', 'services:\n  ${1:app}:\n    image: ${2:image}\n    ports:\n      - "${3:8080}:${3:8080}"'),
  ],
  dockerfile: [
    S('node', 'Node.js image', 'FROM node:${1:22}-alpine\nWORKDIR /app\nCOPY package*.json ./\nRUN npm ci\nCOPY . .\nCMD ["npm", "start"]'),
  ],
  markdown: [
    S('link', 'Link', '[${1:text}](${2:url})'),
    S('code', 'Code block', '```${1:lang}\n$0\n```'),
    S('table', 'Table', '| ${1:A} | ${2:B} |\n| --- | --- |\n| $0 |  |'),
  ],
  hsharp: [S('fn', 'Function', 'fn ${1:name}(${2:args}) -> ${3:void} {\n\t$0\n}'), S('struct', 'Struct', 'struct ${1:Name} {\n\t$0\n}')],
  hackerlang: [S('fn', 'Function', 'fn ${1:name}\n\t$0\nend')],
  hackerscript: [S('fun', 'Function', 'fun ${1:name}(${2:args}) {\n\t$0\n}')],
};

/** Extra keywords for languages without a bank in completionData.ts. */
export const EXTRA_KW: Record<string, string[]> = {
  java: ['public','private','protected','class','interface','enum','extends','implements','static','final','abstract','void','int','long','double','boolean','String','new','return','if','else','for','while','switch','case','try','catch','finally','throw','throws','import','package','this','super','null','true','false','var','record'],
  kotlin: ['fun','val','var','class','object','interface','data','sealed','when','if','else','for','while','return','import','package','null','true','false','is','in','as','by','override','open','suspend','companion','lateinit'],
  swift: ['func','let','var','class','struct','enum','protocol','extension','guard','if','else','for','while','switch','case','return','import','nil','true','false','self','init','throws','try','async','await','some','any','private','public','static'],
  csharp: ['using','namespace','class','struct','interface','enum','public','private','protected','internal','static','void','int','string','bool','var','new','return','if','else','for','foreach','while','switch','case','try','catch','finally','throw','async','await','null','true','false','this','base','record'],
  php: ['function','class','interface','trait','namespace','use','public','private','protected','static','return','if','else','elseif','foreach','for','while','switch','case','try','catch','finally','throw','new','echo','null','true','false','array','isset','empty','require','include'],
  ruby: ['def','end','class','module','if','elsif','else','unless','case','when','while','until','for','in','do','begin','rescue','ensure','return','yield','self','nil','true','false','require','attr_accessor','puts'],
  lua: ['function','local','end','if','then','elseif','else','for','while','do','repeat','until','return','break','nil','true','false','and','or','not','require','pairs','ipairs','print'],
  sql: ['SELECT','FROM','WHERE','INSERT','INTO','VALUES','UPDATE','SET','DELETE','CREATE','TABLE','ALTER','DROP','JOIN','LEFT','RIGHT','INNER','OUTER','ON','GROUP','BY','ORDER','HAVING','LIMIT','AND','OR','NOT','NULL','AS','DISTINCT','UNION','INDEX','PRIMARY','KEY','FOREIGN','REFERENCES'],
  shell: ['if','then','else','elif','fi','for','while','do','done','case','esac','function','return','exit','export','local','echo','cd','ls','grep','sed','awk','cat','set','source','trap'],
  yaml: ['true','false','null','services','version','image','ports','volumes','environment','name','on','jobs','steps','run','uses','with'],
  dockerfile: ['FROM','RUN','CMD','LABEL','EXPOSE','ENV','ADD','COPY','ENTRYPOINT','VOLUME','USER','WORKDIR','ARG','ONBUILD','HEALTHCHECK','SHELL'],
};
