<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { ArrowLeft, Circle, Square } from 'lucide-svelte';
  import { t } from '../lib/stores/language';
  import { classifySwipe } from '../lib/altShell/gestures';

  const dispatch = createEventDispatcher<{ back: void; home: void; recents: void }>();

  // Swiping up on the bar = Home, like gesture navigation on phones.
  let startX = 0; let startY = 0;
  function down(e: PointerEvent) { startX = e.clientX; startY = e.clientY; }
  function up(e: PointerEvent) {
    if (classifySwipe(e.clientX - startX, e.clientY - startY, { minDistance: 30 }) === 'up') dispatch('home');
  }
</script>

<nav class="phone-safe-bottom relative z-30 shrink-0 bg-black/40 backdrop-blur-md text-white"
  aria-label="Navigation" on:pointerdown={down} on:pointerup={up}>
  <div class="h-14 flex items-center justify-around">
    <button class="h-12 w-20 flex items-center justify-center rounded-full active:bg-white/15" aria-label={$t('altui.back')} on:click={() => dispatch('back')}><ArrowLeft size={22} /></button>
    <button class="h-12 w-20 flex items-center justify-center rounded-full active:bg-white/15" aria-label={$t('altui.home')} on:click={() => dispatch('home')}><Circle size={20} /></button>
    <button class="h-12 w-20 flex items-center justify-center rounded-full active:bg-white/15" aria-label={$t('altui.recents')} on:click={() => dispatch('recents')}><Square size={18} /></button>
  </div>
</nav>
