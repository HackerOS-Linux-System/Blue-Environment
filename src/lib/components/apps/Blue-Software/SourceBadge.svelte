<script context="module" lang="ts">
  export const SOURCE_COLORS: Record<string, string> = {
    dnf: 'bg-blue-500/15 text-blue-300 border-blue-500/20',
    flatpak: 'bg-green-500/15 text-green-300 border-green-500/20',
    appimage: 'bg-purple-500/15 text-purple-300 border-purple-500/20',
  };
</script>

<script lang="ts">
  export let source: string;
</script>

<span class="text-[10px] px-1.5 py-0.5 rounded border font-mono {SOURCE_COLORS[source] ?? 'bg-white/5 text-slate-400'}">
  {source}
</span>
