<script lang="ts">
  import { untrack } from 'svelte';
  import type { WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';
  import { t } from '../stores/language';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget }: Props = $props();

  let text = $state(untrack(() => String(widget.config.text ?? '')));
  let timer: ReturnType<typeof setTimeout> | undefined;

  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(() => patchWidget(widget.id, { config: { text } }), 400);
  }
</script>

<textarea
  bind:value={text}
  oninput={onInput}
  onmousedown={(e) => e.stopPropagation()}
  placeholder={$t('widgets.notes_placeholder')}
  spellcheck="false"
  class="w-full h-full resize-none bg-transparent px-3 py-2 text-sm text-amber-100 placeholder:text-slate-500 outline-none"
></textarea>
