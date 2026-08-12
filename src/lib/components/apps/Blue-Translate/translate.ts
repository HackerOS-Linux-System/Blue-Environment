import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { HistoryEntry, TranslateResult } from './types';

const LS_HISTORY = 'blue_translate_history';
const MAX_HISTORY = 100;

function loadHistory(): HistoryEntry[] {
  try { return JSON.parse(localStorage.getItem(LS_HISTORY) ?? '[]'); } catch { return []; }
}

/**
 * Translation backend strategy (in order):
 *  1. A local LibreTranslate instance at http://localhost:5000, if running.
 *  2. Native `translate-shell` via the Rust backend (BlueTranslateApp/mod.rs
 *     — src-tauri/src/BlueTranslateApp/), if the `trans` CLI is installed.
 *     No network service or API key required.
 *  3. MyMemory (mymemory.translated.net) — a free, keyless public HTTP
 *     translation API. Added so the app works out of the box on a fresh
 *     install with only an internet connection: no local server to run,
 *     no CLI to install, no API key to configure. Rate-limited (~5000
 *     words/day per IP without an email param) but plenty for
 *     interactive use.
 *  4. The AI service already configured for Blue AI (Settings → aiConfig).
 *  5. None available → a clear, actionable error.
 */
async function callNativeTranslate(text: string, from: string, to: string): Promise<TranslateResult | null> {
  if (!SystemBridge.isTauri()) return null;
  try {
    const result = await SystemBridge.invokeCommand<string>('translate_text', { text, from, to });
    return result ? { text: result } : null;
  } catch {
    return null;
  }
}

async function callLibreTranslate(text: string, from: string, to: string): Promise<TranslateResult | null> {
  try {
    const res = await fetch('http://localhost:5000/translate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ q: text, source: from === 'auto' ? 'auto' : from, target: to, format: 'text' }),
      signal: AbortSignal.timeout(3000),
    });
    if (!res.ok) return null;
    const data = await res.json();
    if (!data.translatedText) return null;
    return { text: data.translatedText, detectedLang: data.detectedLanguage?.language };
  } catch {
    return null;
  }
}

/** MyMemory caps single requests at 500 bytes; longer text is chunked by
 *  sentence and stitched back together so the app doesn't just fail on
 *  a long paste. */
const MYMEMORY_MAX_CHARS = 480;

function chunkForMyMemory(text: string): string[] {
  if (text.length <= MYMEMORY_MAX_CHARS) return [text];
  const sentences = text.match(/[^.!?\n]+[.!?\n]*/g) ?? [text];
  const chunks: string[] = [];
  let current = '';
  for (const s of sentences) {
    if ((current + s).length > MYMEMORY_MAX_CHARS && current) {
      chunks.push(current);
      current = s;
    } else {
      current += s;
    }
  }
  if (current) chunks.push(current);
  return chunks;
}

async function callMyMemoryTranslate(text: string, from: string, to: string): Promise<TranslateResult | null> {
  try {
    const langpair = `${from === 'auto' ? 'autodetect' : from}|${to}`;
    const chunks = chunkForMyMemory(text);
    const translated: string[] = [];
    let detected: string | undefined;

    for (const chunk of chunks) {
      const url = `https://api.mymemory.translated.net/get?q=${encodeURIComponent(chunk)}&langpair=${encodeURIComponent(langpair)}`;
      const res = await fetch(url, { signal: AbortSignal.timeout(6000) });
      if (!res.ok) return null;
      const data = await res.json();
      const best = data?.responseData?.translatedText;
      if (!best || data?.responseStatus === 403) return null;
      translated.push(best);
      detected = detected ?? data?.responseData?.detectedLanguage;
    }

    return { text: translated.join(' '), detectedLang: detected };
  } catch {
    return null;
  }
}

async function callAiTranslate(text: string, from: string, to: string): Promise<TranslateResult | null> {
  try {
    const cfg = await SystemBridge.loadConfig();
    const aiConfig = (cfg as any)?.aiConfig;
    if (!aiConfig?.apiKey && aiConfig?.service !== 'local') return null;

    const prompt = from === 'auto'
      ? `Translate the following text to ${to}. Reply with ONLY the translation, no explanation, no quotes:\n\n${text}`
      : `Translate the following text from ${from} to ${to}. Reply with ONLY the translation, no explanation, no quotes:\n\n${text}`;

    const reply = await SystemBridge.aiCall({
      service: aiConfig.service, apiKey: aiConfig.apiKey, model: aiConfig.model,
      messages: [{ role: 'user', content: prompt }],
    });
    return { text: reply.trim() };
  } catch {
    return null;
  }
}

export function createTranslateState() {
  const sourceText = writable('');
  const translatedText = writable('');
  const fromLang = writable('auto');
  const toLang = writable('en');
  const loading = writable(false);
  const error = writable<string | null>(null);
  const backend = writable<'libretranslate' | 'native' | 'mymemory' | 'ai' | null>(null);
  const history = writable<HistoryEntry[]>(loadHistory());

  function saveHistory(entry: HistoryEntry) {
    history.update((prev) => {
      const next = [entry, ...prev].slice(0, MAX_HISTORY);
      localStorage.setItem(LS_HISTORY, JSON.stringify(next));
      return next;
    });
  }

  async function translate() {
    const text = get(sourceText).trim();
    if (!text) { translatedText.set(''); return; }

    loading.set(true);
    error.set(null);
    const from = get(fromLang);
    const to = get(toLang);

    let result = await callLibreTranslate(text, from, to);
    if (result) backend.set('libretranslate');
    else {
      result = await callNativeTranslate(text, from, to);
      if (result) backend.set('native');
      else {
        result = await callMyMemoryTranslate(text, from, to);
        if (result) backend.set('mymemory');
        else {
          result = await callAiTranslate(text, from, to);
          if (result) backend.set('ai');
        }
      }
    }

    if (!result) {
      error.set('No translation backend available. Install LibreTranslate locally, or configure an AI service (used by Blue AI) to enable translation.');
      loading.set(false);
      return;
    }

    translatedText.set(result.text);
    saveHistory({ id: Date.now().toString(), from, to, sourceText: text, translatedText: result.text, time: Date.now() });
    loading.set(false);
  }

  function swap() {
    if (get(fromLang) === 'auto') return;
    const f = get(fromLang), t = get(toLang);
    fromLang.set(t); toLang.set(f);
    const s = get(sourceText), tr = get(translatedText);
    sourceText.set(tr); translatedText.set(s);
  }

  function clearHistory() { history.set([]); localStorage.removeItem(LS_HISTORY); }

  function useHistoryEntry(entry: HistoryEntry) {
    fromLang.set(entry.from); toLang.set(entry.to);
    sourceText.set(entry.sourceText); translatedText.set(entry.translatedText);
  }

  return { sourceText, translatedText, fromLang, toLang, loading, error, backend, history, translate, swap, clearHistory, useHistoryEntry };
}
