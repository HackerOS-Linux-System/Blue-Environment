export function lspSnippetToCm(text: string): string {
  return text
    .replace(/\$\{(\d+)\|([^|}]*)\|\}/g, (_m, n: string, choices: string) => `\${${n}:${choices.split(',')[0]}}`)
    .replace(/\$(\d+)/g, '${$1}')
    .replace(/\$\{(\d+)\}/g, '${$1}');
}
