<script lang="ts">
  import { Sparkles } from 'lucide-svelte';
  import { returnToInstaller } from '../utils/liveMode';

  /** Stacking order — App.svelte passes a value just under the Start menu so
   * the button floats above every window but never covers an open menu. */
  export let zIndex = 9000;
  /** Keeps the button clear of the panel when it sits at the bottom. */
  export let panelPosition: 'top' | 'bottom' = 'top';
  export let panelSize = 48;

  $: bottomOffset = 16 + (panelPosition === 'bottom' ? panelSize : 0);
</script>

<!-- Only ever rendered in a live session while the classic desktop is shown
     (see App.svelte). Brings the person back to Blue Installer. -->
<button
  on:click={returnToInstaller}
  title="Go back to Blue Installer"
  class="fixed right-4 flex items-center gap-2 pl-3 pr-4 py-2.5 rounded-2xl text-sm font-medium text-white
         bg-gradient-to-br from-blue-500 to-indigo-700 shadow-xl shadow-blue-900/40 border border-white/15
         hover:brightness-110 active:scale-95 transition"
  style="z-index:{zIndex}; bottom:{bottomOffset}px;"
>
  <Sparkles size={16} />
  Return to Blue Installer
</button>
