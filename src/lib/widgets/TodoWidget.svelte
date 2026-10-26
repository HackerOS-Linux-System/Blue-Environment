<script lang="ts">
  import { Plus, Trash2 } from 'lucide-svelte';
  import { parseTodos, serializeTodos, type TodoItem, type WidgetInstance } from './widgetModel';
  import { patchWidget } from './widgetStore';

  interface Props { widget: WidgetInstance; editing: boolean; }
  let { widget }: Props = $props();

  let items = $derived(parseTodos(widget.config.items));
  let draft = $state('');

  const save = (next: TodoItem[]) => patchWidget(widget.id, { config: { items: serializeTodos(next) } });
  function add() {
    const text = draft.trim();
    if (!text) return;
    save([...items, { id: `t${Date.now().toString(36)}${Math.random().toString(36).slice(2, 5)}`, text, done: false }]);
    draft = '';
  }
  const toggle = (id: string) => save(items.map((i) => (i.id === id ? { ...i, done: !i.done } : i)));
  const remove = (id: string) => save(items.filter((i) => i.id !== id));
  const clearDone = () => save(items.filter((i) => !i.done));
  let doneCount = $derived(items.filter((i) => i.done).length);
</script>

<div class="h-full flex flex-col px-2.5 pt-2 pb-2 gap-1.5" onmousedown={(e) => e.stopPropagation()} role="presentation">
  <div class="flex items-center justify-between text-[11px] text-slate-400 px-0.5">
    <span>{doneCount}/{items.length} done</span>
    {#if doneCount > 0}<button class="hover:text-white" onclick={clearDone}>Clear done</button>{/if}
  </div>
  <div class="flex-1 min-h-0 overflow-y-auto space-y-0.5">
    {#each items as item (item.id)}
      <div class="group flex items-center gap-2 rounded-md px-1.5 py-1 hover:bg-white/5">
        <input type="checkbox" checked={item.done} onchange={() => toggle(item.id)} class="accent-emerald-500" />
        <span class="flex-1 text-sm truncate {item.done ? 'line-through text-slate-500' : 'text-slate-100'}" title={item.text}>{item.text}</span>
        <button class="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400" onclick={() => remove(item.id)} aria-label="Delete"><Trash2 size={12} /></button>
      </div>
    {/each}
    {#if items.length === 0}<p class="text-xs text-slate-500 text-center mt-4">Nothing to do 🎉</p>{/if}
  </div>
  <div class="flex items-center gap-1.5">
    <input bind:value={draft} onkeydown={(e) => { if (e.key === 'Enter') add(); }} placeholder="Add a task…" maxlength="200" class="flex-1 min-w-0 rounded-md bg-black/30 px-2 py-1 text-xs text-slate-100 placeholder:text-slate-500 outline-none" />
    <button class="p-1.5 rounded-md bg-emerald-600/80 hover:bg-emerald-500" onclick={add} aria-label="Add"><Plus size={12} /></button>
  </div>
</div>
