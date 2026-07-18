<script lang="ts">
  import { SystemBridge } from '../utils/systemBridge';
  import { dialogConfirm } from '../stores/dialog';
  import {
    GitBranch, GitCommit, RefreshCw, Check, X, Plus, Minus,
    Upload, Download, AlertCircle, FolderGit2,
  } from 'lucide-svelte';

  export let cwd: string;

  interface GitFile { status: string; staged: boolean; path: string; }
  interface GitCommitInfo { hash: string; author: string; date: string; message: string; }

  let branch = '';
  let files: GitFile[] = [];
  let commits: GitCommitInfo[] = [];
  let commitMsg = '';
  let tab: 'changes' | 'history' | 'diff' = 'changes';
  const TABS = ['changes', 'history', 'diff'] as const;
  let diff = '';
  let selectedFile: string | null = null;
  let loading = false;
  let error: string | null = null;
  let isGitRepo = false;

  async function run(cmd: string): Promise<string> {
    const r = await SystemBridge.executeCommand(`cd "${cwd}" && ${cmd} 2>&1`);
    return typeof r === 'string' ? r : (r as any)?.stdout || '';
  }

  async function refresh() {
    loading = true;
    error = null;
    try {
      const gitCheck = await run('git rev-parse --git-dir');
      if (gitCheck.includes('not a git repository') || !gitCheck.includes('.git')) {
        isGitRepo = false; loading = false; return;
      }
      isGitRepo = true;

      const br = await run('git branch --show-current');
      branch = br.trim() || 'HEAD';

      const status = await run('git status --porcelain');
      const fs: GitFile[] = [];
      for (const line of status.split('\n').filter(Boolean)) {
        const xy = line.slice(0, 2);
        const path = line.slice(3);
        const staged = xy[0] !== ' ' && xy[0] !== '?';
        const wt = xy[1] !== ' ';
        if (staged || wt || xy === '??') fs.push({ status: staged ? xy[0] : xy[1], staged, path: path.trim() });
      }
      files = fs;

      const log = await run('git log --oneline --format="%H|%an|%ar|%s" -15');
      commits = log.split('\n').filter(Boolean).map((l) => {
        const parts = l.split('|');
        return { hash: parts[0]?.slice(0, 7) || '', author: parts[1] || '', date: parts[2] || '', message: parts[3] || '' };
      });
    } catch (e: any) { error = e.message; }
    loading = false;
  }

  $: if (cwd) refresh();

  async function stageFile(path: string) { await run(`git add "${path}"`); refresh(); }
  async function unstageFile(path: string) { await run(`git reset HEAD "${path}"`); refresh(); }
  async function discardFile(path: string) {
    const ok = await dialogConfirm({ title: 'Discard changes', message: `Discard changes to ${path}? This cannot be undone.`, confirmLabel: 'Discard', danger: true });
    if (!ok) return;
    await run(`git checkout -- "${path}"`); refresh();
  }
  async function stageAll() { await run('git add -A'); refresh(); }
  async function commit() {
    if (!commitMsg.trim()) return;
    await run(`git commit -m "${commitMsg.replace(/"/g, '\\"')}"`);
    commitMsg = ''; refresh();
  }
  async function push() { loading = true; const out = await run('git push 2>&1'); error = out.includes('error') ? out : null; loading = false; refresh(); }
  async function pull() { loading = true; const out = await run('git pull 2>&1'); error = out.includes('error') ? out : null; loading = false; refresh(); }
  async function showDiff(path: string) { selectedFile = path; diff = (await run(`git diff HEAD -- "${path}"`)) || '(no diff)'; tab = 'diff'; }

  const STATUS_COLOR: Record<string, string> = { M: 'text-yellow-400', A: 'text-green-400', D: 'text-red-400', '?': 'text-slate-400', R: 'text-blue-400' };

  $: staged = files.filter((f) => f.staged);
  $: unstaged = files.filter((f) => !f.staged);

  function diffLineColor(line: string) {
    if (line.startsWith('+')) return 'text-green-400';
    if (line.startsWith('-')) return 'text-red-400';
    if (line.startsWith('@')) return 'text-blue-400';
    return 'text-slate-300';
  }
</script>

{#if !cwd}
  <div class="flex items-center justify-center h-full text-slate-500 text-sm">
    <div class="text-center"><FolderGit2 size={24} class="mx-auto mb-2" />Open a folder first</div>
  </div>
{:else if !isGitRepo}
  <div class="flex flex-col items-center justify-center h-full text-slate-500 text-sm gap-3">
    <FolderGit2 size={32} class="text-slate-600" />
    <p>Not a Git repository</p>
    <button on:click={() => run('git init').then(refresh)} class="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-white text-xs transition-colors">
      Initialize Repository
    </button>
  </div>
{:else}
  <div class="flex flex-col h-full text-white overflow-hidden">
    <div class="shrink-0 flex items-center justify-between px-3 py-2 border-b border-white/5 bg-slate-800/50">
      <div class="flex items-center gap-2">
        <GitBranch size={14} class="text-blue-400" />
        <span class="text-sm font-medium">{branch}</span>
        {#if loading}<RefreshCw size={12} class="animate-spin text-slate-500" />{/if}
      </div>
      <div class="flex gap-1">
        <button on:click={pull} title="Pull" class="p-1 hover:bg-white/10 rounded text-slate-400 hover:text-white"><Download size={13} /></button>
        <button on:click={push} title="Push" class="p-1 hover:bg-white/10 rounded text-slate-400 hover:text-white"><Upload size={13} /></button>
        <button on:click={refresh} class="p-1 hover:bg-white/10 rounded text-slate-400"><RefreshCw size={12} /></button>
      </div>
    </div>

    <div class="shrink-0 flex border-b border-white/5">
      {#each TABS as t (t)}
        <button on:click={() => (tab = t)}
          class="px-3 py-1.5 text-xs capitalize transition-colors border-b-2 {tab === t ? 'border-blue-500 text-white' : 'border-transparent text-slate-400'}">
          {t === 'changes' ? `Changes (${files.length})` : t === 'history' ? `History (${commits.length})` : 'Diff'}
        </button>
      {/each}
    </div>

    {#if error}
      <div class="shrink-0 px-3 py-2 bg-red-500/10 border-b border-red-500/20 text-red-300 text-xs flex gap-2">
        <AlertCircle size={12} class="shrink-0 mt-0.5" /><span>{error}</span>
        <button on:click={() => (error = null)} class="ml-auto"><X size={10} /></button>
      </div>
    {/if}

    {#if tab === 'changes'}
      <div class="flex-1 overflow-y-auto">
        <div class="p-3 border-b border-white/5 space-y-2">
          <textarea bind:value={commitMsg} placeholder="Commit message..." rows="2"
            class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-xs text-white resize-none focus:outline-none focus:border-blue-500/50 placeholder-slate-600" />
          <div class="flex gap-2">
            <button on:click={stageAll} class="flex-1 py-1 bg-slate-700 hover:bg-slate-600 rounded text-xs transition-colors">Stage All</button>
            <button on:click={commit} disabled={!commitMsg.trim() || staged.length === 0}
              class="flex-1 py-1 bg-blue-600 hover:bg-blue-500 rounded text-xs transition-colors disabled:opacity-40">
              <span class="flex items-center justify-center gap-1"><GitCommit size={11} /> Commit ({staged.length})</span>
            </button>
          </div>
        </div>

        {#if staged.length > 0}
          <div>
            <div class="px-3 py-1.5 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">Staged ({staged.length})</div>
            {#each staged as f (f.path)}
              <div class="flex items-center gap-2 px-3 py-1 hover:bg-white/5 group cursor-pointer" on:click={() => showDiff(f.path)}>
                <span class="text-[10px] font-bold w-4 {STATUS_COLOR[f.status] || 'text-slate-400'}">{f.status}</span>
                <span class="text-xs text-slate-300 flex-1 truncate">{f.path}</span>
                <button on:click={(e) => { e.stopPropagation(); unstageFile(f.path); }} class="opacity-0 group-hover:opacity-100 p-0.5 hover:text-yellow-400 text-slate-600"><Minus size={11} /></button>
              </div>
            {/each}
          </div>
        {/if}

        {#if unstaged.length > 0}
          <div>
            <div class="px-3 py-1.5 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">Changes ({unstaged.length})</div>
            {#each unstaged as f (f.path)}
              <div class="flex items-center gap-2 px-3 py-1 hover:bg-white/5 group cursor-pointer" on:click={() => showDiff(f.path)}>
                <span class="text-[10px] font-bold w-4 {STATUS_COLOR[f.status] || 'text-slate-400'}">{f.status}</span>
                <span class="text-xs text-slate-300 flex-1 truncate">{f.path}</span>
                <div class="flex gap-0.5 opacity-0 group-hover:opacity-100">
                  <button on:click={(e) => { e.stopPropagation(); stageFile(f.path); }} class="p-0.5 hover:text-green-400 text-slate-600"><Plus size={11} /></button>
                  <button on:click={(e) => { e.stopPropagation(); discardFile(f.path); }} class="p-0.5 hover:text-red-400 text-slate-600"><X size={11} /></button>
                </div>
              </div>
            {/each}
          </div>
        {/if}

        {#if files.length === 0}
          <div class="flex items-center justify-center py-8 text-slate-600 text-xs">
            <Check size={14} class="mr-2 text-green-500" />Working tree clean
          </div>
        {/if}
      </div>
    {/if}

    {#if tab === 'history'}
      <div class="flex-1 overflow-y-auto">
        {#each commits as c (c.hash)}
          <div class="px-3 py-2.5 border-b border-white/5 hover:bg-white/5 cursor-pointer"
            on:click={() => run(`git show ${c.hash}`).then((d) => { diff = d; tab = 'diff'; })}>
            <div class="flex items-center gap-2 mb-0.5">
              <code class="text-blue-400 text-[10px] font-mono">{c.hash}</code>
              <span class="text-[10px] text-slate-500 ml-auto">{c.date}</span>
            </div>
            <div class="text-xs text-slate-200 truncate">{c.message}</div>
            <div class="text-[10px] text-slate-500 mt-0.5">{c.author}</div>
          </div>
        {/each}
      </div>
    {/if}

    {#if tab === 'diff'}
      <div class="flex-1 overflow-auto bg-slate-950 p-3">
        {#if selectedFile}<div class="text-xs text-slate-400 mb-2">{selectedFile}</div>{/if}
        <pre class="text-[11px] font-mono leading-5 whitespace-pre-wrap">{#each diff.split('\n') as line, i (i)}<span class={diffLineColor(line)}>{line}{'\n'}</span>{/each}</pre>
      </div>
    {/if}
  </div>
{/if}
