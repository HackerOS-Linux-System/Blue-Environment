export interface Remembered { key: string | null; index: number }

export class FocusMemory {
  private saved = new Map<string, Remembered>();

  remember(scope: string, key: string | null, index: number): void {
    this.saved.set(scope, { key, index });
  }

  recall(scope: string): Remembered | undefined {
    return this.saved.get(scope);
  }

  forget(scope: string): void {
    this.saved.delete(scope);
  }
}

/**
 * Which of the screen's focusable elements to focus. `keys[i]` is the
 * `data-tv-key` of the i-th focusable element (or `null`). Returns `-1` when
 * there is nothing to focus.
 */
export function pickRestoreIndex(keys: (string | null)[], saved: Remembered | undefined): number {
  if (keys.length === 0) return -1;
  if (!saved) return 0;
  if (saved.key !== null) {
    const byKey = keys.indexOf(saved.key);
    if (byKey >= 0) return byKey;
  }
  return Math.min(Math.max(saved.index, 0), keys.length - 1);
}
