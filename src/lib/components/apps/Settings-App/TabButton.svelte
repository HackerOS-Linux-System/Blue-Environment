<script lang="ts">
  interface Props {
    icon: any;
    label: string;
    isActive: boolean;
    onclick?: (e: MouseEvent) => void;
  }

  let { icon, label, isActive, onclick }: Props = $props();

  const SvelteComponent = $derived(icon);
</script>

<button {onclick}
  class="w-full flex items-center gap-3 p-2.5 rounded-lg text-sm font-medium transition-all duration-200 {isActive ? 'bg-blue-600 text-white shadow-lg shadow-blue-500/20 translate-x-1' : 'text-slate-400 hover:bg-white/5 hover:text-white'}">
  <SvelteComponent size={18} /> {label}
</button>
