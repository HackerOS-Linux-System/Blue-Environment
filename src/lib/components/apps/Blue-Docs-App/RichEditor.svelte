<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { colorScheme } from '../../../stores/colorScheme';

  export let content: string;
  export let fontFamily: string;
  export let fontSize: number;

  const dispatch = createEventDispatcher<{ change: string }>();

  let iframeEl: HTMLIFrameElement;
  let styleEl: HTMLStyleElement | null = null;
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
    doc.addEventListener('keydown', (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        if (e.key === 'b') { e.preventDefault(); doc.execCommand('bold'); }
        if (e.key === 'i') { e.preventDefault(); doc.execCommand('italic'); }
        if (e.key === 'u') { e.preventDefault(); doc.execCommand('underline'); }
      }
      if (e.key === 'Tab') { e.preventDefault(); doc.execCommand('insertHTML', false, '&nbsp;&nbsp;&nbsp;&nbsp;'); }
    });
  }

  onMount(() => iframeEl?.addEventListener('load', handleLoad));
  onDestroy(() => iframeEl?.removeEventListener('load', handleLoad));

  $: if (styleEl) styleEl.textContent = buildStyle($colorScheme === 'light');

  $: { const doc = getDoc(); if (doc?.body && doc.body.innerHTML !== content) doc.body.innerHTML = content; }

  $: {
    const doc = getDoc();
    if (doc?.body) {
      doc.body.style.fontFamily = fontFamily === 'Monospace' ? 'monospace' : fontFamily === 'Serif' ? 'serif' : fontFamily === 'System UI' ? 'system-ui, sans-serif' : `"${fontFamily}", system-ui, sans-serif`;
      doc.body.style.fontSize = `${fontSize}px`;
    }
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

<iframe bind:this={iframeEl} src="about:blank" class="flex-1 w-full border-none" title="document editor" style="background:transparent;" />
