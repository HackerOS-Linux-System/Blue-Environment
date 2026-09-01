<script lang="ts">
  // Blue Tasks — new app (not an expansion of an existing one). See
  // src-tauri/src/BlueTasksApp/mod.rs's module doc for the storage
  // model and cross-app integration design (Calendar linking, Blue Web
  // "Save to Blue Tasks").
  import { onMount } from 'svelte';
  import { Plus, Check, Trash2, Calendar as CalendarIcon, Flag, ExternalLink, X, ListChecks, ChevronDown } from 'lucide-svelte';
  import { createTasksStore } from './tasksStore';
  import { LIST_COLORS, PRIORITY_LABELS } from './types';
  import type { Task, TaskList } from './types';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { t } from '../../../stores/language';

  export let windowId: string;
  // Optional launch args — Blue Web's "Save to Blue Tasks" action opens
  // this app (if not already open) with a pre-filled title/sourceUrl via
  // the same `launchArgs` mechanism every app receives as extra props
  // (see App.svelte: `props={{ windowId: win.id, ...win.launchArgs }}`).
  export let prefillTitle: string | undefined = undefined;
  export let prefillUrl: string | undefined = undefined;

  const store = createTasksStore();
  const { lists, tasks, loading, error } = store;

  let selectedListId: string | null = null;
  let showDone = true;
  let editing: Task | null = null;
  let newTaskTitle = '';
  let addingList = false;
  let newListName = '';
  let newListColor: (typeof LIST_COLORS)[number] = LIST_COLORS[0];

  $: visibleLists = $lists;
  $: currentListId = selectedListId ?? visibleLists[0]?.id ?? null;
  $: currentTasks = ($tasks || [])
    .filter((t) => t.listId === currentListId)
    .filter((t) => showDone || !t.done)
    .sort((a, b) => {
      if (a.done !== b.done) return a.done ? 1 : -1;
      if (a.priority !== b.priority) return b.priority - a.priority;
      return (a.dueDate ?? '9999').localeCompare(b.dueDate ?? '9999');
    });
  $: doneCountForList = (id: string) => ($tasks || []).filter((t) => t.listId === id && !t.done).length;

  onMount(async () => {
    await store.load();
    if (prefillTitle) {
      editing = {
        id: '', listId: currentListId ?? visibleLists[0]?.id ?? 'inbox', title: prefillTitle, notes: '',
        done: false, dueDate: null, dueTime: null, priority: 0, linkedEventId: null,
        sourceUrl: prefillUrl ?? null, createdAt: '',
      } as Task;
    }
  });

  async function quickAdd() {
    const title = newTaskTitle.trim();
    if (!title || !currentListId) return;
    newTaskTitle = '';
    await store.upsert({
      listId: currentListId, title, notes: '', done: false, dueDate: null, dueTime: null,
      priority: 0, linkedEventId: null, sourceUrl: null,
    });
  }

  async function toggleDone(task: Task) {
    await store.setDone(task.id, !task.done);
  }

  function openEditor(task: Task) {
    editing = { ...task };
  }

  async function saveEditor() {
    if (!editing) return;
    if (!editing.title.trim()) { editing = null; return; }
    await store.upsert(editing);
    editing = null;
  }

  async function deleteEditing() {
    if (!editing) return;
    if (editing.id) await store.remove(editing.id);
    editing = null;
  }

  /**
   * "Add to Calendar" — calls the existing Blue Calendar backend
   * command directly rather than a Tasks-specific one (see mod.rs's
   * module doc on why). Only enabled once a due date is set; the
   * calendar event's own id is stashed back onto the task so a second
   * click updates the same event instead of creating duplicates.
   */
  async function addToCalendar() {
    if (!editing || !editing.dueDate) return;
    const event = {
      id: editing.linkedEventId || `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      title: editing.title,
      date: editing.dueDate,
      time: editing.dueTime || '',
      notes: editing.notes,
      color: visibleLists.find((l) => l.id === editing!.listId)?.color ?? '#3b82f6',
    };
    const res = await SystemBridge.calendarSaveEvent(event as any);
    if (res.ok) editing = { ...editing, linkedEventId: event.id };
  }

  async function addList() {
    const name = newListName.trim();
    if (!name) { addingList = false; return; }
    const list = await store.saveList({ name, color: newListColor });
    selectedListId = list.id;
    newListName = '';
    addingList = false;
  }

  async function removeList(id: string) {
    if (visibleLists.length <= 1) return;
    await store.deleteList(id);
    if (currentListId === id) selectedListId = visibleLists.find((l) => l.id !== id)?.id ?? null;
  }

  function priorityColor(p: number): string {
    return p === 3 ? 'text-red-400' : p === 2 ? 'text-amber-400' : p === 1 ? 'text-blue-400' : 'text-slate-600';
  }

  function fmtDue(t: Task): string {
    if (!t.dueDate) return '';
    const d = new Date(t.dueDate + 'T00:00:00');
    const s = d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
    return t.dueTime ? `${s}, ${t.dueTime}` : s;
  }

  function isOverdue(t: Task): boolean {
    if (!t.dueDate || t.done) return false;
    return t.dueDate < new Date().toISOString().slice(0, 10);
  }
</script>

<div class="flex h-full bg-slate-900 text-white text-sm">
  <!-- Lists sidebar -->
  <div class="w-52 shrink-0 border-r border-white/5 flex flex-col bg-slate-950/40">
    <div class="flex items-center gap-2 px-3 h-11 border-b border-white/5 shrink-0">
      <ListChecks size={16} class="text-blue-400" />
      <span class="font-semibold">{$t('tasks.title')}</span>
    </div>
    <div class="flex-1 overflow-y-auto py-1">
      {#each visibleLists as list (list.id)}
        <button on:click={() => (selectedListId = list.id)}
          class="group w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-white/5 {currentListId === list.id ? 'bg-white/10' : ''}">
          <span class="w-2.5 h-2.5 rounded-full shrink-0" style="background:{list.color}" />
          <span class="flex-1 truncate">{list.name}</span>
          {#if doneCountForList(list.id) > 0}<span class="text-[10px] text-slate-500">{doneCountForList(list.id)}</span>{/if}
          {#if visibleLists.length > 1}
            <button on:click={(e) => { e.stopPropagation(); removeList(list.id); }} class="opacity-0 group-hover:opacity-100 hover:text-red-400"><X size={12} /></button>
          {/if}
        </button>
      {/each}
    </div>
    <div class="p-2 border-t border-white/5 shrink-0">
      {#if addingList}
        <div class="flex items-center gap-1.5">
          <input bind:value={newListName} on:keydown={(e) => e.key === 'Enter' && addList()} autofocus
            placeholder={$t('tasks.new_list_placeholder')} class="flex-1 bg-slate-800 rounded-md px-2 py-1 text-xs focus:outline-none" />
          <div class="flex gap-1">
            {#each LIST_COLORS as c}
              <button on:click={() => (newListColor = c)} class="w-3.5 h-3.5 rounded-full {newListColor === c ? 'ring-2 ring-white' : ''}" style="background:{c}" />
            {/each}
          </div>
        </div>
      {:else}
        <button on:click={() => (addingList = true)} class="w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md text-slate-400 hover:bg-white/5 hover:text-white text-xs">
          <Plus size={13} /> {$t('tasks.new_list')}
        </button>
      {/if}
    </div>
  </div>

  <!-- Task list -->
  <div class="flex-1 flex flex-col min-w-0">
    <div class="flex items-center justify-between px-4 h-11 border-b border-white/5 shrink-0">
      <span class="font-medium truncate">{visibleLists.find((l) => l.id === currentListId)?.name ?? ''}</span>
      <button on:click={() => (showDone = !showDone)} class="text-xs text-slate-400 hover:text-white flex items-center gap-1">
        {showDone ? $t('tasks.hide_done') : $t('tasks.show_done')} <ChevronDown size={12} />
      </button>
    </div>

    <div class="px-4 py-2 border-b border-white/5 shrink-0">
      <div class="flex items-center gap-2 bg-slate-800 rounded-lg px-3 py-2">
        <Plus size={14} class="text-slate-500 shrink-0" />
        <input bind:value={newTaskTitle} on:keydown={(e) => e.key === 'Enter' && quickAdd()}
          placeholder={$t('tasks.add_placeholder')} class="flex-1 bg-transparent focus:outline-none placeholder:text-slate-500" />
      </div>
    </div>

    <div class="flex-1 overflow-y-auto">
      {#if $loading}
        <div class="flex items-center justify-center h-full text-slate-500 text-xs">{$t('tasks.loading')}</div>
      {:else if currentTasks.length === 0}
        <div class="flex flex-col items-center justify-center h-full text-slate-500 gap-2">
          <ListChecks size={28} class="opacity-30" />
          <span class="text-xs">{$t('tasks.empty')}</span>
        </div>
      {:else}
        {#each currentTasks as task (task.id)}
          <div class="group flex items-start gap-3 px-4 py-2.5 hover:bg-white/5 border-b border-white/[0.03]">
            <button on:click={() => toggleDone(task)}
              class="mt-0.5 w-4 h-4 rounded-full border shrink-0 flex items-center justify-center transition-colors {task.done ? 'bg-blue-500 border-blue-500' : 'border-slate-600 hover:border-blue-400'}">
              {#if task.done}<Check size={11} class="text-white" />{/if}
            </button>
            <button on:click={() => openEditor(task)} class="flex-1 min-w-0 text-left">
              <div class="truncate {task.done ? 'line-through text-slate-500' : 'text-white'}">{task.title}</div>
              <div class="flex items-center gap-2 mt-0.5">
                {#if task.dueDate}
                  <span class="text-[10px] flex items-center gap-1 {isOverdue(task) ? 'text-red-400' : 'text-slate-500'}">
                    <CalendarIcon size={10} /> {fmtDue(task)}
                  </span>
                {/if}
                {#if task.priority > 0}<Flag size={10} class={priorityColor(task.priority)} />{/if}
                {#if task.sourceUrl}<ExternalLink size={10} class="text-slate-600" />{/if}
              </div>
            </button>
            <button on:click={() => store.remove(task.id)} class="opacity-0 group-hover:opacity-100 p-1 hover:text-red-400 shrink-0"><Trash2 size={13} /></button>
          </div>
        {/each}
      {/if}
    </div>
  </div>

  <!-- Editor panel -->
  {#if editing}
    <div class="w-80 shrink-0 border-l border-white/5 flex flex-col bg-slate-950/40">
      <div class="flex items-center justify-between px-4 h-11 border-b border-white/5 shrink-0">
        <span class="font-medium text-xs text-slate-400">{$t('tasks.details')}</span>
        <button on:click={() => (editing = null)}><X size={14} /></button>
      </div>
      <div class="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
        <input bind:value={editing.title} placeholder={$t('tasks.title_placeholder')} class="bg-slate-800 rounded-lg px-3 py-2 font-medium focus:outline-none" />

        {#if editing.sourceUrl}
          <a href={editing.sourceUrl} target="_blank" rel="noopener" class="text-xs text-blue-400 hover:underline flex items-center gap-1 truncate">
            <ExternalLink size={11} /> {editing.sourceUrl}
          </a>
        {/if}

        <div>
          <span class="text-[10px] text-slate-500 uppercase tracking-wide">{$t('tasks.list')}</span>
          <select bind:value={editing.listId} class="w-full mt-1 bg-slate-800 rounded-lg px-3 py-1.5 focus:outline-none">
            {#each visibleLists as l}<option value={l.id}>{l.name}</option>{/each}
          </select>
        </div>

        <div class="flex gap-2">
          <div class="flex-1">
            <span class="text-[10px] text-slate-500 uppercase tracking-wide">{$t('tasks.due_date')}</span>
            <input type="date" bind:value={editing.dueDate} class="w-full mt-1 bg-slate-800 rounded-lg px-3 py-1.5 focus:outline-none" />
          </div>
          <div class="w-28">
            <span class="text-[10px] text-slate-500 uppercase tracking-wide">{$t('tasks.due_time')}</span>
            <input type="time" bind:value={editing.dueTime} disabled={!editing.dueDate} class="w-full mt-1 bg-slate-800 rounded-lg px-3 py-1.5 focus:outline-none disabled:opacity-40" />
          </div>
        </div>

        {#if editing.dueDate}
          <button on:click={addToCalendar} class="text-xs flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-white/5 hover:bg-white/10 self-start">
            <CalendarIcon size={12} class={editing.linkedEventId ? 'text-green-400' : 'text-slate-400'} />
            {editing.linkedEventId ? $t('tasks.calendar_linked') : $t('tasks.add_to_calendar')}
          </button>
        {/if}

        <div>
          <span class="text-[10px] text-slate-500 uppercase tracking-wide">{$t('tasks.priority')}</span>
          <div class="flex gap-1.5 mt-1">
            {#each [0, 1, 2, 3] as p}
              <button on:click={() => { if (editing) editing.priority = p; }}
                class="flex-1 py-1.5 rounded-lg text-xs flex items-center justify-center gap-1 {editing.priority === p ? 'bg-blue-500/20 ring-1 ring-blue-500/50' : 'bg-slate-800 hover:bg-slate-700'}">
                <Flag size={11} class={priorityColor(p)} /> {$t(PRIORITY_LABELS[p])}
              </button>
            {/each}
          </div>
        </div>

        <div class="flex-1 flex flex-col">
          <span class="text-[10px] text-slate-500 uppercase tracking-wide">{$t('tasks.notes')}</span>
          <textarea bind:value={editing.notes} rows="6" placeholder={$t('tasks.notes_placeholder')}
            class="mt-1 flex-1 bg-slate-800 rounded-lg px-3 py-2 resize-none focus:outline-none text-slate-200" />
        </div>
      </div>
      <div class="flex items-center justify-between p-3 border-t border-white/5 shrink-0">
        <button on:click={deleteEditing} class="text-xs text-red-400 hover:text-red-300 flex items-center gap-1"><Trash2 size={12} /> {$t('tasks.delete')}</button>
        <button on:click={saveEditor} class="text-xs px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 font-medium">{$t('tasks.save')}</button>
      </div>
    </div>
  {/if}
</div>
