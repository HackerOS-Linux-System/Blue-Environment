import { writable, get } from 'svelte/store';
import type { DocFile, DocFormat } from './types';
import { SystemBridge } from '../../../utils/systemBridge';

const AUTOSAVE_INTERVAL = 20_000;

function makeDoc(format: DocFormat = 'rich'): DocFile {
  const now = new Date();
  return {
    id: `doc-${Date.now()}`,
    name: `Untitled ${format === 'spreadsheet' ? 'Spreadsheet' : format === 'presentation' ? 'Presentation' : 'Document'}`,
    format,
    content: format === 'spreadsheet' ? JSON.stringify([Array(10).fill({ value: '' })]) : '',
    modified: false, created: now, updated: now,
  };
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
      const expandedPath = doc.path.startsWith('~/') ? doc.path.replace('~', '$HOME') : doc.path;
      await SystemBridge.executeCommand(`mkdir -p "$(dirname '${expandedPath}')" && printf '%s' ${JSON.stringify(doc.content)} > '${expandedPath}'`);
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
