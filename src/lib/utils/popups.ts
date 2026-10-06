export const CLOSE_POPUPS_EVENT = 'blue:close-popups';

export function closePopups(): void {
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent(CLOSE_POPUPS_EVENT));
}
