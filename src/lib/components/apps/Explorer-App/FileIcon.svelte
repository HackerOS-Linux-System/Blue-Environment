<script lang="ts">
  import { onMount } from 'svelte';
  import { Folder, Image, Music, Video, FileText, Archive, Code, File } from 'lucide-svelte';
  import type { FileEntry } from './types';
  import {
    fileTypeAssociations, ensureFileTypeAssociationsLoaded,
    resolveAssociationLocally, ICON_COMPONENTS,
  } from '../../../utils/fileTypeAssociations';

  export let file: FileEntry;
  export let size = 40;

  $: ext = file.name.split('.').pop()?.toLowerCase() ?? '';
  $: codeExt = ['rs', 'ts', 'tsx', 'js', 'jsx', 'py', 'go', 'c', 'cpp'].includes(ext);

  // User-defined associations (Settings > Custom File Types) always win
  // over the built-in rules below — this is the "let users add icon
  // support for their own file formats/MIME types" feature this
  // component previously had no hook for at all (a closed if/else
  // chain with nowhere for a user preference to plug in).
  onMount(() => { ensureFileTypeAssociationsLoaded(); });
  $: customMatch = file.is_dir ? null : resolveAssociationLocally(file.name, file.mime_type, $fileTypeAssociations);
  $: CustomIcon = customMatch ? ICON_COMPONENTS[customMatch.icon] : null;
</script>

{#if customMatch && CustomIcon}
  <svelte:component this={CustomIcon} {size} style="color: {customMatch.color}" />
{:else if file.is_dir}
  <Folder {size} class="text-blue-400" />
{:else if file.mime_type.startsWith('image/')}
  <Image {size} class="text-green-400" />
{:else if file.mime_type.startsWith('audio/')}
  <Music {size} class="text-purple-400" />
{:else if file.mime_type.startsWith('video/')}
  <Video {size} class="text-red-400" />
{:else if file.mime_type.startsWith('text/') || file.mime_type.includes('json') || file.mime_type.includes('xml')}
  <FileText {size} class="text-yellow-400" />
{:else if file.mime_type.includes('zip') || file.mime_type.includes('tar') || file.mime_type.includes('gz')}
  <Archive {size} class="text-orange-400" />
{:else if codeExt}
  <Code {size} class="text-cyan-400" />
{:else}
  <File {size} class="text-slate-400" />
{/if}
