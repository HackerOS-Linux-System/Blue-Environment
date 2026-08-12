<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';
  import { Target } from 'lucide-svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const GRID_SIZE = 9; // 3x3
  const ROUND_MS = 30000;

  let activeCell: number | null = null;
  let score = 0;
  let timeLeft = ROUND_MS;
  let running = false;
  let over = false;
  let spawnTimer: ReturnType<typeof setTimeout> | undefined;
  let tickTimer: ReturnType<typeof setInterval> | undefined;
  let moleSpawnedAt = 0;
  let reactionTimes: number[] = [];
  let missCount = 0;

  function scheduleNextMole() {
    const delay = 400 + Math.random() * 700;
    spawnTimer = setTimeout(() => {
      let next: number;
      do { next = Math.floor(Math.random() * GRID_SIZE); } while (next === activeCell);
      activeCell = next;
      moleSpawnedAt = Date.now();
      // If not clicked within this window, it "escapes" — counts as a miss.
      spawnTimer = setTimeout(() => {
        if (activeCell !== null) {
          missCount++;
          activeCell = null;
          scheduleNextMole();
        }
      }, 900);
    }, delay);
  }

  function whack(i: number) {
    if (!running || i !== activeCell) return;
    const reaction = Date.now() - moleSpawnedAt;
    reactionTimes.push(reaction);
    activeCell = null;
    clearTimeout(spawnTimer);
    score += Math.max(10, 200 - Math.floor(reaction / 5));
    dispatch('score', score);
    scheduleNextMole();
  }

  function start() {
    score = 0; timeLeft = ROUND_MS; running = true; over = false;
    activeCell = null; reactionTimes = []; missCount = 0;
    dispatch('score', score);
    scheduleNextMole();
    tickTimer = setInterval(() => {
      timeLeft -= 100;
      if (timeLeft <= 0) endGame();
    }, 100);
  }

  function endGame() {
    running = false;
    over = true;
    clearTimeout(spawnTimer);
    clearInterval(tickTimer);
    activeCell = null;
    dispatch('gameOver', score);
  }

  onDestroy(() => { clearTimeout(spawnTimer); clearInterval(tickTimer); });

  $: avgReaction = reactionTimes.length > 0 ? Math.round(reactionTimes.reduce((a, b) => a + b, 0) / reactionTimes.length) : 0;
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    {#if running}<span>Time: {(timeLeft / 1000).toFixed(1)}s</span>{/if}
    {#if !running}
      <button on:click={start} class="px-3 py-1 bg-blue-600 hover:bg-blue-500 rounded-lg text-xs font-medium">{over ? 'Play again' : 'Start'}</button>
    {/if}
  </div>
  <div class="grid grid-cols-3 gap-2 bg-slate-800 p-3 rounded-2xl border border-white/10">
    {#each Array(GRID_SIZE) as _, i (i)}
      <button on:click={() => whack(i)}
        class="w-20 h-20 rounded-xl flex items-center justify-center transition-colors {activeCell === i ? 'bg-amber-500' : 'bg-slate-900'}">
        {#if activeCell === i}<Target size={28} class="text-slate-950" />{/if}
      </button>
    {/each}
  </div>
  {#if over}
    <p class="text-sm text-slate-300">Avg reaction: <strong class="text-emerald-400">{avgReaction}ms</strong> · Missed: {missCount}</p>
  {/if}
  <p class="text-xs text-slate-500">Click the target the instant it appears. 30 second rounds.</p>
</div>
