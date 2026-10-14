export function focusOnMount(node: HTMLElement, enabled: boolean = true): void {
  if (!enabled) return;
  queueMicrotask(() => {
    node.focus();
    if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) node.select?.();
  });
}

/**
 * Attaches a `keydown` listener from an action. For container elements whose
 * role is "dialog"/"list": they legitimately handle Escape/arrow keys for
 * everything inside them, but Svelte's linter (rightly) doesn't expect
 * `on:keydown` on a non-interactive role, so the listener is added here.
 */
export function onKeydown(node: HTMLElement, handler: (e: KeyboardEvent) => void) {
  let current = handler;
  const listener = (e: KeyboardEvent) => current(e);
  node.addEventListener('keydown', listener);
  return {
    update(next: (e: KeyboardEvent) => void) { current = next; },
    destroy() { node.removeEventListener('keydown', listener); },
  };
}
