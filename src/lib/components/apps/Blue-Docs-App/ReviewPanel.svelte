<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, X, CheckCheck, Plus, Trash2, MessageSquarePlus, Users } from 'lucide-svelte';
  import { parseReview } from './docReview';
  import { profileState, loadProfiles, updateProfiles, addProfile, removeProfile, activeAuthor, PROFILE_COLORS } from './docsProfile';

  interface Props {
    /** Current document HTML (the lists are derived from it). */
    content: string;
    trackChanges: boolean;
    commentDraft: string;
    onToggleTracking: () => void;
    onAddComment: (text: string) => void;
    onReply: (id: string, text: string) => void;
    onResolve: (id: string, resolved: boolean) => void;
    onDeleteComment: (id: string) => void;
    onChange: (action: 'accept' | 'reject' | 'acceptAll' | 'rejectAll', id?: string) => void;
    onFocusComment: (id: string) => void;
  }
  let { content, trackChanges, commentDraft = $bindable(), onToggleTracking, onAddComment, onReply, onResolve, onDeleteComment, onChange, onFocusComment }: Props = $props();

  let review = $derived(parseReview(content));
  let author = $derived(activeAuthor($profileState));
  let replyText = $state<Record<string, string>>({});
  let newProfile = $state('');
  let showProfiles = $state(false);
  let showResolved = $state(true);

  onMount(loadProfiles);

  const when = (ts: number) => (ts ? new Date(ts).toLocaleString([], { dateStyle: 'short', timeStyle: 'short' }) : '');
  function submitComment() { if (commentDraft.trim()) { onAddComment(commentDraft); commentDraft = ''; } }
  function submitReply(id: string) { const t = (replyText[id] ?? '').trim(); if (t) { onReply(id, t); replyText = { ...replyText, [id]: '' }; } }
</script>

<aside class="w-72 shrink-0 border-l border-white/5 bg-slate-900/60 flex flex-col overflow-hidden text-sm" aria-label="Review">
  <!-- author profile -->
  <div class="p-3 border-b border-white/5">
    <button onclick={() => (showProfiles = !showProfiles)} class="w-full flex items-center gap-2 text-left">
      <span class="w-6 h-6 rounded-full flex items-center justify-center text-[11px] font-semibold text-white" style="background:{author.color}">{author.name.slice(0, 1).toUpperCase()}</span>
      <span class="flex-1 truncate text-slate-200">{author.name}</span>
      <Users size={14} class="text-slate-500" />
    </button>
    {#if showProfiles}
      <div class="mt-2 space-y-1">
        {#each $profileState.profiles as p (p.id)}
          <div class="flex items-center gap-2">
            <button onclick={() => updateProfiles((s) => ({ ...s, activeId: p.id }))} class="flex-1 flex items-center gap-2 px-2 py-1 rounded-lg text-left {p.id === $profileState.activeId ? 'bg-blue-600/30 text-white' : 'text-slate-400 hover:bg-white/5'}">
              <span class="w-3 h-3 rounded-full" style="background:{p.color}"></span><span class="truncate">{p.name}</span>
            </button>
            {#if $profileState.profiles.length > 1}
              <button onclick={() => updateProfiles((s) => removeProfile(s, p.id))} class="p-1 text-slate-500 hover:text-red-400" title="Remove profile" aria-label="Remove profile"><Trash2 size={12} /></button>
            {/if}
          </div>
        {/each}
        <form class="flex gap-1 pt-1" onsubmit={(e) => { e.preventDefault(); updateProfiles((s) => addProfile(s, newProfile)); newProfile = ''; }}>
          <input bind:value={newProfile} placeholder="New author…" class="flex-1 min-w-0 bg-slate-800 border border-white/10 rounded-lg px-2 py-1 text-xs text-white" />
          <button class="p-1.5 rounded-lg bg-blue-600 text-white" title="Add profile" aria-label="Add profile"><Plus size={12} /></button>
        </form>
      </div>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto">
    <!-- track changes -->
    <div class="p-3 border-b border-white/5">
      <div class="flex items-center justify-between mb-2">
        <span class="font-medium text-slate-200">Track changes</span>
        <button role="switch" aria-checked={trackChanges} aria-label="Track changes" onclick={onToggleTracking}
          class="w-9 h-5 rounded-full relative transition-colors {trackChanges ? 'bg-blue-600' : 'bg-slate-600'}">
          <span class="absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform {trackChanges ? 'translate-x-4' : ''}"></span>
        </button>
      </div>
      {#if review.changes.length}
        <div class="flex gap-1 mb-2">
          <button onclick={() => onChange('acceptAll')} class="flex-1 flex items-center justify-center gap-1 px-2 py-1 rounded-lg bg-emerald-600/20 text-emerald-300 hover:bg-emerald-600/30 text-xs"><CheckCheck size={12} /> Accept all</button>
          <button onclick={() => onChange('rejectAll')} class="flex-1 flex items-center justify-center gap-1 px-2 py-1 rounded-lg bg-red-600/20 text-red-300 hover:bg-red-600/30 text-xs"><X size={12} /> Reject all</button>
        </div>
        <ul class="space-y-1.5">
          {#each review.changes as c (c.id)}
            <li class="rounded-lg bg-slate-800 p-2">
              <div class="flex items-center gap-1.5 text-[11px] text-slate-500 mb-0.5">
                <span class="w-2 h-2 rounded-full" style="background:{c.color || PROFILE_COLORS[0]}"></span>
                <span class="truncate">{c.author}</span><span class="ml-auto">{when(c.ts)}</span>
              </div>
              <div class="text-xs {c.type === 'insert' ? 'text-emerald-300' : 'text-red-300 line-through'} break-words">
                {c.type === 'insert' ? '+' : '−'} {c.text}
              </div>
              <div class="flex gap-1 mt-1">
                <button onclick={() => onChange('accept', c.id)} class="p-1 rounded hover:bg-emerald-600/20 text-emerald-400" title="Accept" aria-label="Accept change"><Check size={12} /></button>
                <button onclick={() => onChange('reject', c.id)} class="p-1 rounded hover:bg-red-600/20 text-red-400" title="Reject" aria-label="Reject change"><X size={12} /></button>
              </div>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="text-xs text-slate-500">No tracked changes.</p>
      {/if}
    </div>

    <!-- comments -->
    <div class="p-3">
      <div class="flex items-center justify-between mb-2">
        <span class="font-medium text-slate-200">Comments ({review.comments.length})</span>
        <label class="flex items-center gap-1 text-[11px] text-slate-500"><input type="checkbox" bind:checked={showResolved} /> resolved</label>
      </div>
      <div class="mb-3">
        <textarea bind:value={commentDraft} placeholder="Select text, then write a comment…" rows="2"
          class="w-full resize-none bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-xs text-white placeholder:text-slate-500"></textarea>
        <button onclick={submitComment} disabled={!commentDraft.trim()} class="mt-1 w-full flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-white text-xs"><MessageSquarePlus size={13} /> Comment on selection</button>
      </div>
      <ul class="space-y-2">
        {#each review.comments.filter((c) => showResolved || !c.resolved) as c (c.id)}
          <li class="rounded-lg bg-slate-800 p-2 {c.resolved ? 'opacity-60' : ''}">
            <button class="block w-full text-left text-[11px] text-yellow-300/80 italic truncate mb-1" onclick={() => onFocusComment(c.id)} title="Show in document">“{c.quote}”</button>
            <div class="text-[11px] text-slate-500">{c.author} · {when(c.ts)}</div>
            <div class="text-xs text-slate-200 break-words">{c.text}</div>
            {#each c.replies as r}
              <div class="mt-1.5 pl-2 border-l border-white/10">
                <div class="text-[11px] text-slate-500">{r.a} · {when(r.t)}</div>
                <div class="text-xs text-slate-300 break-words">{r.x}</div>
              </div>
            {/each}
            <form class="flex gap-1 mt-1.5" onsubmit={(e) => { e.preventDefault(); submitReply(c.id); }}>
              <input bind:value={replyText[c.id]} placeholder="Reply…" class="flex-1 min-w-0 bg-slate-900 border border-white/10 rounded px-2 py-0.5 text-xs text-white" />
            </form>
            <div class="flex gap-2 mt-1 text-[11px]">
              <button onclick={() => onResolve(c.id, !c.resolved)} class="text-slate-400 hover:text-emerald-300">{c.resolved ? 'Reopen' : 'Resolve'}</button>
              <button onclick={() => onDeleteComment(c.id)} class="text-slate-500 hover:text-red-400">Delete</button>
            </div>
          </li>
        {/each}
        {#if review.comments.length === 0}<li class="text-xs text-slate-500">No comments yet.</li>{/if}
      </ul>
    </div>
  </div>
</aside>
