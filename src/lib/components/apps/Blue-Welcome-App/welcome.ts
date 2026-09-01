const KEY = 'blue-welcome-completed';

export function hasCompletedWelcome(): boolean {
  try {
    return localStorage.getItem(KEY) === '1';
  } catch {
    // Storage disabled/unavailable (rare, but shouldn't crash startup)
    // — treat as "already seen" so a broken localStorage can't force
    // the wizard open on every single boot.
    return true;
  }
}

export function markWelcomeCompleted(): void {
  try { localStorage.setItem(KEY, '1'); } catch { /* best effort */ }
}
