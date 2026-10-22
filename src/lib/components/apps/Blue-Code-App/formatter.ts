type Plugins = () => Promise<unknown[]>;

interface Fmt { parser: string; plugins: Plugins; }

const babel: Plugins = async () => [(await import('prettier/plugins/babel')).default, (await import('prettier/plugins/estree')).default];
const ts: Plugins = async () => [(await import('prettier/plugins/typescript')).default, (await import('prettier/plugins/estree')).default];
const postcss: Plugins = async () => [(await import('prettier/plugins/postcss')).default];
const html: Plugins = async () => [(await import('prettier/plugins/html')).default];
const md: Plugins = async () => [(await import('prettier/plugins/markdown')).default];
const yaml: Plugins = async () => [(await import('prettier/plugins/yaml')).default];

const FORMATTERS: Record<string, Fmt> = {
  javascript: { parser: 'babel', plugins: babel },
  typescript: { parser: 'typescript', plugins: ts },
  json: { parser: 'json', plugins: babel },
  css: { parser: 'css', plugins: postcss },
  scss: { parser: 'scss', plugins: postcss },
  less: { parser: 'less', plugins: postcss },
  html: { parser: 'html', plugins: html },
  vue: { parser: 'vue', plugins: html },
  markdown: { parser: 'markdown', plugins: md },
  yaml: { parser: 'yaml', plugins: yaml },
};

export function canFormat(language: string): boolean {
  return language in FORMATTERS;
}

export interface FormatOptions { tabSize: number; }

/** Returns the formatted text, or throws with a readable message. */
export async function formatSource(language: string, text: string, opts: FormatOptions): Promise<string> {
  const f = FORMATTERS[language];
  if (!f) throw new Error(`No formatter for ${language}`);
  const prettier = await import('prettier/standalone');
  return prettier.format(text, {
    parser: f.parser, plugins: (await f.plugins()) as any[],
    tabWidth: opts.tabSize, singleQuote: true, printWidth: 100,
  });
}
