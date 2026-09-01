export interface TaskList {
  id: string;
  name: string;
  color: string;
}

export interface Task {
  id: string;
  listId: string;
  title: string;
  notes: string;
  done: boolean;
  dueDate: string | null;
  dueTime: string | null;
  /** 0 = none, 1 = low, 2 = medium, 3 = high. */
  priority: number;
  linkedEventId: string | null;
  sourceUrl: string | null;
  createdAt: string;
}

export const LIST_COLORS = ['#3b82f6', '#22c55e', '#f59e0b', '#ef4444', '#a855f7', '#06b6d4'] as const;

export const PRIORITY_LABELS = ['tasks.priority.none', 'tasks.priority.low', 'tasks.priority.medium', 'tasks.priority.high'] as const;
