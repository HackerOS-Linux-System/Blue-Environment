<script lang="ts">
  import { Edit3, Eye } from 'lucide-svelte';
  import { createEventDispatcher } from 'svelte';

  export let content: string;
  const dispatch = createEventDispatcher<{ change: string }>();

  let preview = false;

  function renderMd(md: string): string {
    return md
      .replace(/^### (.+)$/gm, '<h3 style="font-size:1.1rem;font-weight:600;margin:.75rem 0 .25rem">$1</h3>')
      .replace(/^## (.+)$/gm, '<h2 style="font-size:1.3rem;font-weight:600;margin:1rem 0 .4rem">$1</h2>')
      .replace(/^# (.+)$/gm, '<h1 style="font-size:1.7rem;font-weight:600;margin:1.2rem 0 .5rem">$1</h1>')
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>')
      .replace(/`([^`]+)`/g, '<code style="font-family:monospace;background:rgba(255,255,255,.08);padding:.1em .3em;border-radius:.2em">$1</code>')
      .replace(/^> (.+)$/gm, '<blockquote style="border-left:3px solid #3b82f6;margin:.5rem 0;padding:.3rem .8rem;color:#94a3b8;font-style:italic">$1</blockquote>')
      .replace(/^- (.+)$/gm, '<li style="margin:.2rem 0">$1</li>')
      .replace(/(<li[^>]*>.+<\/li>\n?)+/g, '<ul style="padding-left:1.5rem;margin:.5rem 0">$&</ul>')
      .replace(/^\d+\. (.+)$/gm, '<li style="margin:.2rem 0">$1</li>')
      .replace(/^---$/gm, '<hr style="border:none;border-top:1px solid rgba(255,255,255,.1);margin:1rem 0">')
      .replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2" style="color:#60a5fa;text-decoration:underline">$1</a>')
      .replace(/\n\n/g, '</p><p style="margin:.5rem 0">')
      .replace(/^(.+)$/gm, (m) => (m.startsWith('<') ? m : `<p style="margin:.5rem 0">${m}</p>`));
  }
</script>

<div class="flex-1 flex flex-col overflow-hidden">
  <div class="shrink-0 flex items-center gap-1 px-3 py-1.5 border-b border-white/5 bg-slate-900/50">
    <button on:click={() => (preview = false)} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-colors {!preview ? 'bg-blue-600/25 text-blue-300' : 'text-slate-500 hover:text-white'}"><Edit3 size={12} /> Edit</button>
    <button on:click={() => (preview = true)} class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-colors {preview ? 'bg-blue-600/25 text-blue-300' : 'text-slate-500 hover:text-white'}"><Eye size={12} /> Preview</button>
  </div>

  {#if preview}
    <div class="flex-1 overflow-y-auto p-8" style="color:#e2e8f0; font-family:system-ui,sans-serif; line-height:1.7;">{@html renderMd(content)}</div>
  {:else}
    <textarea value={content} on:input={(e) => dispatch('change', e.currentTarget.value)}
      class="flex-1 bg-transparent text-slate-200 p-8 resize-none focus:outline-none text-sm leading-relaxed font-mono"
      placeholder="# Your document title&#10;&#10;Start writing in Markdown…" spellcheck="false"
      style="font-family:'JetBrains Mono','Fira Code',monospace;" />
  {/if}
</div>
