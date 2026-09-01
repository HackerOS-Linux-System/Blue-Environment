import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import type { Task, TaskList } from './types';

function newId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function createTasksStore() {
  const lists = writable<TaskList[]>([]);
  const tasks = writable<Task[]>([]);
  const loading = writable(true);
  const error = writable<string | null>(null);

  async function load() {
    loading.set(true);
    try {
      const [l, t] = await Promise.all([SystemBridge.tasksLoadLists(), SystemBridge.tasksLoadTasks()]);
      lists.set(l);
      tasks.set(t);
    } finally {
      loading.set(false);
    }
  }

  async function saveList(list: Omit<TaskList, 'id'> & { id?: string }): Promise<TaskList> {
    const full: TaskList = { ...list, id: list.id ?? newId() };
    const res = await SystemBridge.tasksSaveList(full);
    if (!res.ok) { error.set(res.error ?? 'Failed to save list'); return full; }
    lists.update((prev) => {
      const idx = prev.findIndex((x) => x.id === full.id);
      if (idx >= 0) { const next = [...prev]; next[idx] = full; return next; }
      return [...prev, full];
    });
    return full;
  }

  async function deleteList(id: string) {
    const res = await SystemBridge.tasksDeleteList(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to delete list'); return; }
    lists.update((prev) => prev.filter((l) => l.id !== id));
    tasks.update((prev) => prev.filter((t) => t.listId !== id));
  }

  /**
   * `sourceUrl` is how Blue Web's "Save to Blue Tasks" action creates a
   * task — see BlueWebApp.svelte's address-bar action, which calls this
   * exact function (not a separate web-specific path) with the current
   * page's URL and title.
   */
  async function upsert(task: Omit<Task, 'id' | 'createdAt'> & { id?: string; createdAt?: string }): Promise<Task> {
    const full: Task = {
      ...task,
      id: task.id ?? newId(),
      createdAt: task.createdAt ?? new Date().toISOString(),
    };
    const res = await SystemBridge.tasksUpsert(full);
    if (!res.ok) { error.set(res.error ?? 'Failed to save task'); return full; }
    tasks.update((prev) => {
      const idx = prev.findIndex((x) => x.id === full.id);
      if (idx >= 0) { const next = [...prev]; next[idx] = full; return next; }
      return [...prev, full];
    });
    return full;
  }

  async function remove(id: string) {
    const res = await SystemBridge.tasksDelete(id);
    if (!res.ok) { error.set(res.error ?? 'Failed to delete task'); return; }
    tasks.update((prev) => prev.filter((t) => t.id !== id));
  }

  async function setDone(id: string, done: boolean) {
    // Optimistic — flip locally immediately (checkbox toggling should
    // feel instant), then persist; a failure re-reads from the backend
    // rather than trying to hand-roll a rollback of a single boolean.
    tasks.update((prev) => prev.map((t) => (t.id === id ? { ...t, done } : t)));
    const res = await SystemBridge.tasksSetDone(id, done);
    if (!res.ok) { error.set(res.error ?? 'Failed to update task'); await load(); }
  }

  function tasksIn(listId: string): Task[] {
    return get(tasks).filter((t) => t.listId === listId);
  }

  return { lists, tasks, loading, error, load, saveList, deleteList, upsert, remove, setDone, tasksIn };
}

export type TasksStore = ReturnType<typeof createTasksStore>;
