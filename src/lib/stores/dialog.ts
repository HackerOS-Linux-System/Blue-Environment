import { writable } from 'svelte/store';

export interface PromptOptions { title: string; label?: string; defaultValue?: string; placeholder?: string; confirmLabel?: string; cancelLabel?: string; inputType?: 'text' | 'password'; }
export interface ConfirmOptions { title: string; message: string; confirmLabel?: string; cancelLabel?: string; danger?: boolean; }
export interface AlertOptions { title: string; message: string; confirmLabel?: string; }

interface PendingPrompt { kind: 'prompt'; options: PromptOptions; resolve: (v: string | null) => void; }
interface PendingConfirm { kind: 'confirm'; options: ConfirmOptions; resolve: (v: boolean) => void; }
interface PendingAlert { kind: 'alert'; options: AlertOptions; resolve: () => void; }
export type PendingDialog = PendingPrompt | PendingConfirm | PendingAlert;

export const activeDialog = writable<PendingDialog | null>(null);

/** Show an in-shell text input dialog. Resolves to the trimmed string, or null if cancelled. */
export function dialogPrompt(options: PromptOptions): Promise<string | null> {
  return new Promise((resolve) => activeDialog.set({ kind: 'prompt', options, resolve }));
}

/** Show an in-shell yes/no confirmation dialog. Resolves to true/false. */
export function dialogConfirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => activeDialog.set({ kind: 'confirm', options, resolve }));
}

/** Show an in-shell single-button informational dialog (replaces window.alert). */
export function dialogAlert(options: AlertOptions): Promise<void> {
  return new Promise((resolve) => activeDialog.set({ kind: 'alert', options, resolve: () => resolve() }));
}

export function closeDialog(current: PendingDialog, result: string | boolean | null) {
  if (current.kind === 'prompt') current.resolve(result as string | null);
  else if (current.kind === 'confirm') current.resolve(result as boolean);
  else current.resolve();
  activeDialog.set(null);
}
