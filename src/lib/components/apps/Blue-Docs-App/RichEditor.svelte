<script lang="ts">
  import { run } from 'svelte/legacy';

  import { onMount, onDestroy } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { colorScheme } from '../../../stores/colorScheme';
  import {
    REVIEW_CSS, addCommentToRange, replyToComment, setCommentResolved, deleteComment, trackedInsert, trackedDeleteRange,
    acceptChange, rejectChange, acceptAll, rejectAll, type Author,
  } from './docReview';

  interface Props {
    content: string;
    fontFamily: string;
    fontSize: number;
    /** When on, typing/deleting is recorded as tracked changes by `author` instead of editing directly. */
    trackChanges?: boolean;
    author?: Author;
  }

  let { content, fontFamily, fontSize, trackChanges = false, author = { name: 'Me', color: '#3b82f6' } }: Props = $props();

  const dispatch = createEventDispatcher<{ change: string }>();

  let iframeEl = $state() as HTMLIFrameElement;
  let styleEl = $state<HTMLStyleElement | null>(null);
  let initialised = false;

  // This editor's content lives inside an <iframe> (needed for a real,
  // isolated `designMode`/`contenteditable` document) — which means it's
  // a genuinely separate document the parent page's `app.css` can never
  // reach, no matter how the CSS override block outside is written. The
  // stylesheet has to be built and re-injected into *this* document
  // directly, which is what `buildStyle()`/the reactive block below do
  // whenever `colorScheme` changes — this was the actual reason Docs'
  // rich-text editor kept rendering near-white text on a light
  // background (readable on the old always-dark surface, unreadable
  // once the surface around it turned light).
  function buildStyle(isLight: boolean): string {
    const text = isLight ? '#1e293b' : '#e2e8f0';
    const quote = isLight ? '#475569' : '#94a3b8';
    const quoteBg = isLight ? 'rgba(59,130,246,.06)' : 'rgba(59,130,246,.05)';
    const codeBg = isLight ? 'rgba(15,23,42,.06)' : 'rgba(255,255,255,.08)';
    const preBg = isLight ? 'rgba(15,23,42,.05)' : 'rgba(0,0,0,.3)';
    const hrColor = isLight ? 'rgba(15,23,42,.12)' : 'rgba(255,255,255,.1)';
    const tableBorder = isLight ? 'rgba(15,23,42,.15)' : 'rgba(255,255,255,.12)';
    const thBg = isLight ? 'rgba(15,23,42,.05)' : 'rgba(255,255,255,.05)';
    return `
    *{box-sizing:border-box}
    body{margin:0;padding:32px 48px;font-size:14px;color:${text};font-family:system-ui,sans-serif;
         background:transparent;line-height:1.7;min-height:100vh;outline:none}
    body:focus{outline:none}
    h1{font-size:2rem;font-weight:600;margin:0 0 1rem}
    h2{font-size:1.5rem;font-weight:600;margin:0 0 .75rem}
    h3{font-size:1.25rem;font-weight:600;margin:0 0 .5rem}
    p{margin:0 0 .75rem}
    ul,ol{margin:0 0 .75rem;padding-left:1.5rem}
    li{margin:.25rem 0}
    blockquote{border-left:3px solid #3b82f6;margin:.75rem 0;padding:.5rem 1rem;
               color:${quote};font-style:italic;background:${quoteBg}}
    code{font-family:monospace;background:${codeBg};padding:.1em .3em;border-radius:.25em}
    pre{background:${preBg};padding:1rem;border-radius:.5rem;overflow:auto}
    a{color:#60a5fa;text-decoration:underline}
    hr{border:none;border-top:1px solid ${hrColor};margin:1.5rem 0}
    table{border-collapse:collapse;width:100%;margin:.75rem 0}
    td,th{border:1px solid ${tableBorder};padding:.4rem .6rem}
    th{background:${thBg};font-weight:600}
    ::selection{background:#1d4ed8;color:#fff}
    mark{background:#ca8a04;color:#000;border-radius:.2em;padding:.05em .2em}
    ${REVIEW_CSS}
  `;
  }

  function getDoc(): Document | null { return iframeEl?.contentDocument ?? null; }

  function handleLoad() {
    const doc = getDoc();
    if (!doc || initialised) return;
    initialised = true;

    doc.open();
    doc.write(`<!DOCTYPE html><html><head></head><body contenteditable="true" spellcheck="true">${content}</body></html>`);
    doc.close();
    doc.designMode = 'on';

    styleEl = doc.createElement('style');
    styleEl.textContent = buildStyle($colorScheme === 'light');
    doc.head.appendChild(styleEl);

    doc.body.addEventListener('input', () => dispatch('change', doc.body.innerHTML));
    doc.body.addEventListener('beforeinput', onBeforeInput as EventListener);
    doc.addEventListener('keydown', (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        if (e.key === 'b') { e.preventDefault(); doc.execCommand('bold'); }
        if (e.key === 'i') { e.preventDefault(); doc.execCommand('italic'); }
        if (e.key === 'u') { e.preventDefault(); doc.execCommand('underline'); }
      }
      if (e.key === 'Tab') { e.preventDefault(); doc.execCommand('insertHTML', false, '&nbsp;&nbsp;&nbsp;&nbsp;'); }
    });
  }

  const emit = () => { const d = getDoc(); if (d) dispatch('change', d.body.innerHTML); };

  // Track-changes interception: typing becomes <ins>, deleting becomes <del> (nothing is lost).
  function onBeforeInput(e: InputEvent) {
    if (!trackChanges) return;
    const d = getDoc();
    if (!d) return;
    const type = e.inputType;
    if (type === 'insertText' && e.data) {
      e.preventDefault();
      if (trackedInsert(d, e.data, author)) emit();
    } else if (type === 'insertFromPaste' || type === 'insertFromDrop') {
      const text = e.dataTransfer?.getData('text/plain');
      if (text) { e.preventDefault(); if (trackedInsert(d, text, author)) emit(); }
    } else if (type.startsWith('delete')) {
      const sel = d.getSelection();
      if (!sel || sel.rangeCount === 0) return;
      const backward = type.includes('Backward');
      if (sel.isCollapsed) {
        const unit = type.includes('Word') ? 'word' : type.includes('Line') ? 'line' : 'character';
        (sel as Selection & { modify?: (a: string, b: string, c: string) => void }).modify?.('extend', backward ? 'backward' : 'forward', unit);
      }
      const range = sel.getRangeAt(0);
      if (range.collapsed) return;
      e.preventDefault();
      trackedDeleteRange(d, range, author, Date.now(), backward ? 'before' : 'after');
      emit();
    }
  }

  onMount(() => iframeEl?.addEventListener('load', handleLoad));
  onDestroy(() => iframeEl?.removeEventListener('load', handleLoad));

  run(() => {
    if (styleEl) styleEl.textContent = buildStyle($colorScheme === 'light');
  });

  run(() => { const doc = getDoc(); if (doc?.body && doc.body.innerHTML !== content) doc.body.innerHTML = content; });

  run(() => {
    const doc = getDoc();
    if (doc?.body) {
      doc.body.style.fontFamily = fontFamily === 'Monospace' ? 'monospace' : fontFamily === 'Serif' ? 'serif' : fontFamily === 'System UI' ? 'system-ui, sans-serif' : `"${fontFamily}", system-ui, sans-serif`;
      doc.body.style.fontSize = `${fontSize}px`;
    }
  });

  // ── review API (used by ReviewPanel through BlueDocsApp) ──
  export function addComment(text: string): boolean {
    const d = getDoc();
    const sel = d?.getSelection();
    if (!d || !sel || sel.rangeCount === 0) return false;
    const ok = addCommentToRange(d, sel.getRangeAt(0), author, text) !== null;
    if (ok) emit();
    return ok;
  }
  export function replyComment(id: string, text: string) { const d = getDoc(); if (d && replyToComment(d.body, id, author, text)) emit(); }
  export function resolveComment(id: string, resolved: boolean) { const d = getDoc(); if (d && setCommentResolved(d.body, id, resolved)) emit(); }
  export function removeComment(id: string) { const d = getDoc(); if (d && deleteComment(d.body, id)) emit(); }
  export function changeAction(action: 'accept' | 'reject' | 'acceptAll' | 'rejectAll', id?: string) {
    const d = getDoc();
    if (!d) return;
    if (action === 'acceptAll') acceptAll(d.body);
    else if (action === 'rejectAll') rejectAll(d.body);
    else if (id) (action === 'accept' ? acceptChange : rejectChange)(d.body, id);
    emit();
  }
  export function focusComment(id: string) {
    const d = getDoc();
    const el = d && [...d.querySelectorAll<HTMLElement>('span.bd-comment')].find((e) => e.getAttribute('data-comment-id') === id);
    if (!el) return;
    el.scrollIntoView({ block: 'center', behavior: 'smooth' });
    el.classList.add('bd-flash');
    setTimeout(() => el.classList.remove('bd-flash'), 1200);
  }

  export function applyFormat(cmd: string, val?: string) {
    const doc = getDoc();
    if (!doc) return;
    doc.body.focus();

    switch (cmd) {
      case 'bold': doc.execCommand('bold'); break;
      case 'italic': doc.execCommand('italic'); break;
      case 'underline': doc.execCommand('underline'); break;
      case 'strikethrough': doc.execCommand('strikeThrough'); break;
      case 'code': doc.execCommand('insertHTML', false, `<code>&ZeroWidthSpace;</code>`); break;
      case 'highlight': doc.execCommand('hiliteColor', false, '#ca8a04'); break;
      case 'align': doc.execCommand(`justify${val === 'left' ? 'Left' : val === 'center' ? 'Center' : val === 'right' ? 'Right' : 'Full'}`); break;
      case 'heading': doc.execCommand('formatBlock', false, `h${val}`); break;
      case 'paragraph': doc.execCommand('formatBlock', false, 'p'); break;
      case 'bullet': doc.execCommand('insertUnorderedList'); break;
      case 'ordered': doc.execCommand('insertOrderedList'); break;
      case 'quote': doc.execCommand('formatBlock', false, 'blockquote'); break;
      case 'hr': doc.execCommand('insertHorizontalRule'); break;
      case 'link': { const url = window.prompt('URL:'); if (url) doc.execCommand('createLink', false, url); break; }
      case 'table': {
        let html = '<table>';
        for (let r = 0; r < 3; r++) { html += '<tr>'; for (let c = 0; c < 3; c++) html += r === 0 ? '<th>&nbsp;</th>' : '<td>&nbsp;</td>'; html += '</tr>'; }
        html += '</table><p>&nbsp;</p>';
        doc.execCommand('insertHTML', false, html);
        break;
      }
    }
    dispatch('change', doc.body.innerHTML);
  }
</script>

<iframe bind:this={iframeEl} src="about:blank" class="flex-1 w-full border-none" title="document editor" style="background:transparent;"></iframe>
