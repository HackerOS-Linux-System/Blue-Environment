import { writable, get } from 'svelte/store';
import type { DocFile, DocFormat } from './types';
import { emptyPresentation } from './types';
import { SystemBridge } from '../../../utils/systemBridge';
import { exportPptx, importPptx } from './pptxFile';

const AUTOSAVE_INTERVAL = 20_000;

function makeDoc(format: DocFormat = 'rich'): DocFile {
  const now = new Date();
  return {
    id: `doc-${Date.now()}`,
    name: `Untitled ${format === 'spreadsheet' ? 'Spreadsheet' : format === 'presentation' ? 'Presentation' : 'Document'}`,
    format,
    content: format === 'spreadsheet' ? JSON.stringify([Array(10).fill({ value: '' })])
      : format === 'presentation' ? JSON.stringify(emptyPresentation())
      : '',
    modified: false, created: now, updated: now,
  };
}

/** `data:...;base64,...` → raw bytes, for feeding a real `.pptx`'s
 * binary content (read via `SystemBridge.readFileAsDataURL`, since the
 * plain `readFile` bridge method is text-only) into `importPptx`. */
function dataUrlToArrayBuffer(dataUrl: string): ArrayBuffer {
  const base64 = dataUrl.split(',')[1] ?? '';
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

/** Inverse direction: `exportPptx`'s `Blob` → a data URL, for
 * `SystemBridge.saveFile` (which writes binary content given as a data
 * URL — the same bridge method the file manager's "save image" flow
 * already uses). */
function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

interface History { past: string[]; future: string[]; }

export function createDocumentState() {
  const first = makeDoc();
  const docs = writable<DocFile[]>([first]);
  const activeId = writable(first.id);
  const history = writable<Record<string, History>>({});

  const activeDoc = () => get(docs).find((d) => d.id === get(activeId)) ?? get(docs)[0];

  setInterval(() => {
    get(docs).forEach((doc) => { if (doc.modified && doc.path) saveDoc(doc.id); });
  }, AUTOSAVE_INTERVAL);

  function pushHistory(id: string, prev: string) {
    history.update((h) => ({ ...h, [id]: { past: [...(h[id]?.past ?? []).slice(-49), prev], future: [] } }));
  }

  function undo() {
    const id = get(activeId);
    const hist = get(history)[id];
    if (!hist || hist.past.length === 0) return;
    const prev = hist.past[hist.past.length - 1];
    const cur = activeDoc().content;
    history.update((h) => ({ ...h, [id]: { past: hist.past.slice(0, -1), future: [cur, ...hist.future] } }));
    docs.update((ds) => ds.map((d) => (d.id === id ? { ...d, content: prev, modified: true } : d)));
  }

  function redo() {
    const id = get(activeId);
    const hist = get(history)[id];
    if (!hist || hist.future.length === 0) return;
    const next = hist.future[0];
    const cur = activeDoc().content;
    history.update((h) => ({ ...h, [id]: { past: [...hist.past, cur], future: hist.future.slice(1) } }));
    docs.update((ds) => ds.map((d) => (d.id === id ? { ...d, content: next, modified: true } : d)));
  }

  function newDoc(format: DocFormat = 'rich') {
    const doc = makeDoc(format);
    docs.update((ds) => [...ds, doc]);
    activeId.set(doc.id);
  }

  function closeDoc(id: string) {
    const current = get(docs);
    const next = current.filter((d) => d.id !== id);
    docs.set(next.length === 0 ? [makeDoc()] : next);
    if (get(activeId) === id) {
      const remaining = current.filter((d) => d.id !== id);
      activeId.set(remaining[remaining.length - 1]?.id ?? get(docs)[0].id);
    }
  }

  function updateContent(content: string) {
    const id = get(activeId);
    pushHistory(id, activeDoc().content);
    docs.update((ds) => ds.map((d) => (d.id === id ? { ...d, content, modified: true, updated: new Date() } : d)));
  }

  function renameDoc(id: string, name: string) {
    docs.update((ds) => ds.map((d) => (d.id === id ? { ...d, name, modified: true } : d)));
  }

  async function saveDoc(id?: string): Promise<boolean> {
    const doc = get(docs).find((d) => d.id === (id ?? get(activeId)));
    if (!doc || !doc.path) return false;
    try {
      if (doc.format === 'presentation' && doc.path.toLowerCase().endsWith('.pptx')) {
        // Real binary .pptx, not this app's internal JSON slide model —
        // see pptxFile.ts's module doc for exactly what round-trips
        // (title/bullet text with **bold**/*italic*, background color,
        // images — not layouts, tables, notes, or anything richer).
        const presentation = JSON.parse(doc.content);
        const blob = await exportPptx(presentation);
        const dataUrl = await blobToDataUrl(blob);
        await SystemBridge.saveFile(doc.path, dataUrl);
      } else {
        const expandedPath = doc.path.startsWith('~/') ? doc.path.replace('~', '$HOME') : doc.path;
        await SystemBridge.executeCommand(`mkdir -p "$(dirname '${expandedPath}')" && printf '%s' ${JSON.stringify(doc.content)} > '${expandedPath}'`);
      }
      docs.update((ds) => ds.map((d) => (d.id === doc.id ? { ...d, modified: false } : d)));
      return true;
    } catch { return false; }
  }

  function saveDocAs(path: string) {
    const id = get(activeId);
    docs.update((ds) => ds.map((d) => (d.id === id ? { ...d, path, name: path.split('/').pop() ?? d.name } : d)));
    setTimeout(() => saveDoc(id), 50);
  }

  async function openDocFromPath(path: string) {
    try {
      const name = path.split('/').pop() ?? 'Untitled';
      const ext = name.split('.').pop()?.toLowerCase() ?? '';
      const format: DocFormat = ext === 'md' ? 'markdown' : ext === 'csv' || ext === 'xlsx' ? 'spreadsheet' : ext === 'pptx' ? 'presentation' : 'rich';

      let content = '';
      if (ext === 'docx') {
        content = await SystemBridge.invokeCommand<string>('docs_read_docx', { path }).catch(() => '<p>DOCX import requires the Blue-Docs backend to be available.</p>');
      } else if (ext === 'pdf') {
        content = await SystemBridge.invokeCommand<string>('docs_read_pdf', { path }).catch(() => '<p>PDF text extraction requires the Blue-Docs backend to be available.</p>');
      } else if (ext === 'pptx') {
        // Real .pptx import via pptxFile.ts (JSZip + OOXML parsing) —
        // see that module's doc for exactly what round-trips. Reading
        // as a data URL (not the plain text `readFile` bridge method)
        // since a .pptx is a binary zip, not text.
        try {
          const dataUrl = await SystemBridge.readFileAsDataURL(path);
          if (!dataUrl) throw new Error('could not read file contents');
          const presentation = await importPptx(dataUrlToArrayBuffer(dataUrl));
          content = JSON.stringify(presentation);
        } catch (e) {
          // Malformed/unsupported .pptx (or the read itself failed) —
          // open an honest empty deck with an explanatory first slide
          // rather than crashing the whole "open file" action.
          content = JSON.stringify({
            slides: [{
              id: `slide-${Date.now()}`,
              title: name.replace(/\.pptx$/i, ''),
              bullets: [`Couldn't read this .pptx: ${e instanceof Error ? e.message : String(e)}`],
              background: '#0f172a',
              images: [],
            }],
          });
        }
      } else {
        content = (await SystemBridge.readFile(path)) ?? '';
      }

      const doc: DocFile = { id: `doc-${Date.now()}`, name, format, content, path, modified: false, created: new Date(), updated: new Date() };
      docs.update((ds) => [...ds, doc]);
      activeId.set(doc.id);
    } catch (e) { console.error('Open failed:', e); }
  }

  return {
    docs, activeId, history,
    newDoc, closeDoc, updateContent, renameDoc, saveDoc, saveDocAs, openDocFromPath, undo, redo,
  };
}

export type DocumentState = ReturnType<typeof createDocumentState>;
