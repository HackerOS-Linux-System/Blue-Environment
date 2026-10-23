<script lang="ts">
  import { run } from 'svelte/legacy';

  import { Loader2 } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import type { FileEntry } from './types';
  import FileIcon from './FileIcon.svelte';
  import { createEventDispatcher } from 'svelte';

  interface Props {
    path: string;
    thumbnails: Record<string, string>;
  }

  let { path, thumbnails }: Props = $props();

  const dispatch = createEventDispatcher<{ navigate: string }>();

  let files = $state<FileEntry[]>([]);
  let loading = $state(false);


  function loadFiles(p: string) {
    loading = true;
    SystemBridge.getFiles(p).then((f: FileEntry[]) => { files = f; loading = false; }).catch(() => (loading = false));
  }

  run(() => {
    loadFiles(path);
  });
  let visible = $derived(files.filter((f) => !f.name.startsWith('.')));
</script>

<div class="flex-1 overflow-auto p-2">
  {#if loading}
    <Loader2 size={16} class="animate-spin text-blue-400 m-4" />
  {:else}
    <div class="grid gap-1" style="grid-template-columns:repeat(auto-fill, minmax(72px, 1fr));">
      {#each visible as file (file.path)}
        <div role="button" tabindex="0" ondblclick={() => file.is_dir && dispatch('navigate', file.path)} onkeydown={(e) => { if (e.key === 'Enter' && file.is_dir) dispatch('navigate', file.path); }} class="flex flex-col items-center p-1.5 rounded-lg hover:bg-white/5 cursor-pointer">
          {#if file.mime_type.startsWith('image/') && thumbnails[file.path]}
            <img src={thumbnails[file.path]} alt="" class="w-10 h-10 object-cover rounded" />
          {:else}
            <FileIcon {file} size={32} />
          {/if}
          <span class="text-[10px] text-center line-clamp-2 mt-1 leading-tight">{file.name}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
