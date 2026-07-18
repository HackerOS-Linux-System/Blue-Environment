<script context="module" lang="ts">
  export function autoDismiss(node: HTMLElement, params: { id: string; onDismiss: (id: string) => void }) {
    const timer = setTimeout(() => params.onDismiss(params.id), 5000);
    return { destroy: () => clearTimeout(timer) };
  }
</script>

<script lang="ts">
  import { X, Bell } from 'lucide-svelte';
  import { onMount, onDestroy } from 'svelte';

  interface Toast { id: string; title: string; message: string; }

  let toasts: Toast[] = [];

  function handler(e: Event) {
    const notif = (e as CustomEvent).detail;
    toasts = [...toasts, { id: notif.id || Date.now().toString(), title: notif.title, message: notif.message }];
  }

  function removeToast(id: string) {
    toasts = toasts.filter((t) => t.id !== id);
  }

  onMount(() => window.addEventListener('blue:show-toast', handler));
  onDestroy(() => window.removeEventListener('blue:show-toast', handler));
</script>

{#if toasts.length > 0}
  <div class="fixed bottom-4 right-4 z-[60] flex flex-col gap-2">
    {#each toasts as toast (toast.id)}
      <div class="w-80 bg-slate-900/95 backdrop-blur border border-white/10 rounded-xl shadow-2xl p-4"
           use:autoDismiss={{ id: toast.id, onDismiss: removeToast }}>
        <div class="flex items-start justify-between gap-2">
          <div class="flex items-start gap-2 flex-1 min-w-0">
            <Bell size={14} class="text-blue-400 mt-0.5 shrink-0" />
            <div class="flex-1 min-w-0">
              <div class="font-semibold text-white text-sm truncate">{toast.title}</div>
              <div class="text-xs text-slate-300 mt-0.5 line-clamp-2">{toast.message}</div>
            </div>
          </div>
          <button on:click={() => removeToast(toast.id)} class="text-slate-400 hover:text-white shrink-0 p-0.5 rounded hover:bg-white/10 transition-colors">
            <X size={14} />
          </button>
        </div>
        <div class="mt-2 h-0.5 bg-slate-700 rounded-full overflow-hidden">
          <div class="h-full bg-blue-500 rounded-full" style="animation: toast-progress 5s linear forwards;" />
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  @keyframes toast-progress {
    from { width: 100%; }
    to { width: 0%; }
  }
</style>
