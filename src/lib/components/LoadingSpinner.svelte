<script lang="ts">
  // A small, reusable "this is doing backend work" indicator — meant
  // for any spot in the shell that calls into Tauri and might take a
  // moment (network access, file scanning, image processing, ...)
  // instead of that call silently doing nothing visible until it
  // resolves. The wallpaper grid's per-tile spinner (see
  // DisplaySection.svelte) is one example this same pattern applies
  // to; this component exists so every other slow-call spot in the
  // shell doesn't need to hand-roll its own spinner markup.
  //
  // Usage:
  //   <LoadingSpinner label="Checking for updates…" />
  //   <LoadingSpinner size={16} inline />
  export let label: string = '';
  export let size: number = 20;
  /// `true` renders compactly on one line next to other content
  /// (a button, a list row); `false` (default) centers in whatever
  /// container it's placed in — for a section that's *entirely*
  /// waiting on one backend call, not just one small piece of it.
  export let inline: boolean = false;
</script>

{#if inline}
  <span class="inline-flex items-center gap-1.5 text-slate-400" role="status" aria-live="polite">
    <svg
      class="animate-spin shrink-0"
      style="width:{size}px;height:{size}px"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" opacity="0.25" />
      <path d="M22 12a10 10 0 0 0-10-10" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
    </svg>
    {#if label}<span class="text-xs">{label}</span>{/if}
  </span>
{:else}
  <div class="flex flex-col items-center justify-center gap-2 py-8 text-slate-400" role="status" aria-live="polite">
    <svg
      class="animate-spin"
      style="width:{size}px;height:{size}px"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" opacity="0.25" />
      <path d="M22 12a10 10 0 0 0-10-10" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
    </svg>
    {#if label}<span class="text-xs">{label}</span>{/if}
  </div>
{/if}
