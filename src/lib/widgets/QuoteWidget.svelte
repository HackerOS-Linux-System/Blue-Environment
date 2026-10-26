<script lang="ts">
  import { dailyIndex, parseQuotes, DEFAULT_QUOTES } from './widgetModel';
  import type { WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget, editing }: Props = $props();

  let quotes = $derived(parseQuotes(String(widget.config.quotes ?? DEFAULT_QUOTES.join('\n'))));
  let offset = $state(0);
  let random = $derived(widget.config.mode === 'random');
  let index = $derived(quotes.length ? (((random ? 0 : dailyIndex(quotes.length)) + offset) % quotes.length + quotes.length) % quotes.length : 0);
  let quote = $derived(quotes[index]);

  const onQuotes = (e: Event) => patchWidget(widget.id, { config: { quotes: (e.currentTarget as HTMLTextAreaElement).value } });
</script>

{#if editing}
  <textarea
    value={String(widget.config.quotes ?? DEFAULT_QUOTES.join('\n'))}
    onchange={onQuotes}
    onmousedown={(e) => e.stopPropagation()}
    spellcheck="false" placeholder="One quote per line: text — author"
    class="mt-6 w-[calc(100%-1rem)] h-[calc(100%-2rem)] mx-2 resize-none rounded bg-black/30 px-2 py-1 text-xs text-slate-200 outline-none"
  ></textarea>
{:else}
  <button
    class="h-full w-full flex flex-col justify-center gap-2 px-4 text-left"
    onclick={() => (offset += 1)}
    title="Next quote"
  >
    {#if quote}
      <p class="text-sm italic leading-snug text-slate-100">“{quote.text}”</p>
      {#if quote.author}<p class="text-xs text-pink-300/90">— {quote.author}</p>{/if}
    {:else}
      <p class="text-xs text-slate-400">No quotes yet — add some with “Edit widgets”.</p>
    {/if}
  </button>
{/if}
