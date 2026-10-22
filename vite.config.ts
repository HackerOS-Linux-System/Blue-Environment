import { defineConfig, type Plugin } from 'vite';
import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

// ── Which shell UI to build ───────────────────────────────────────────────
// VITE_BLUE_UI selects the entry point the frontend boots from:
//   default (or unset) → src/main.ts      desktop shell (computers)
//   phones             → src/phones/main.ts   touch shell for phones
//   tv                 → src/tv/main.ts       10-foot shell for TVs (remote / game pad)
// The entry is swapped in index.html, so the files of the variants you did NOT
// pick are never imported — a regular (default) build contains no phone/TV
// code at all. Set it via `hl build.hl shell [phones|tv]`, the npm scripts
// `build:phones` / `build:tv` (+ `build:tauri:*`), or by hand.
const UI_ENTRIES = {
  default: '/main.ts',
  phones: '/phones/main.ts',
  tv: '/tv/main.ts',
} as const;
type BlueUi = keyof typeof UI_ENTRIES;

const requestedUi = (process.env.VITE_BLUE_UI || 'default').trim().toLowerCase();
if (!(requestedUi in UI_ENTRIES)) {
  throw new Error(
    `Unknown VITE_BLUE_UI="${process.env.VITE_BLUE_UI}". Use one of: ${Object.keys(UI_ENTRIES).join(', ')}.`,
  );
}
const BLUE_UI = requestedUi as BlueUi;

function blueUiEntry(): Plugin {
  return {
    name: 'blue-ui-entry',
    // `order: 'pre'` is essential: it must run BEFORE Vite resolves the
    // <script> into the module graph (afterwards the src is already a hashed
    // asset and the swap would silently do nothing).
    transformIndexHtml: {
      order: 'pre',
      handler(html) {
        if (BLUE_UI === 'default') return html;
        if (!html.includes('src="/main.ts"')) {
          throw new Error('blue-ui-entry: index.html no longer references /main.ts, cannot switch UI.');
        }
        return html.replace('src="/main.ts"', `src="${UI_ENTRIES[BLUE_UI]}"`);
      },
    },
  };
}

export default defineConfig({
  plugins: [svelte({ preprocess: vitePreprocess() }), blueUiEntry()],
  root: 'src',
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: 'localhost',
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    outDir: resolve(__dirname, 'dist'),
    emptyOutDir: true,
    target: process.env.TAURI_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        manualChunks: {
          'svelte-vendor': ['svelte'],
          lucide: ['lucide-svelte'],
          codemirror: ['@codemirror/state', '@codemirror/view', '@codemirror/language', '@codemirror/commands', '@codemirror/autocomplete', '@codemirror/search', '@codemirror/lint'],
        },
      },
    },
  },
});
