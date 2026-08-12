<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const ICONS = ['🚀', '🎮', '🎧', '💎', '🔥', '⚡', '🌙', '🍀', '🎯', '🧩'];
  const PAIRS = 8; // 16 cards, 4x4

  interface Card { icon: string; flipped: boolean; matched: boolean; }

  let cards: Card[] = [];
  let firstPick: number | null = null;
  let secondPick: number | null = null;
  let moves = 0;
  let matches = 0;
  let locked = false;
  let startTime = 0;
  let score = 0;
  let won = false;

  function shuffle<T>(arr: T[]): T[] {
    const a = [...arr];
    for (let i = a.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
  }

  function reset() {
    const chosen = shuffle(ICONS).slice(0, PAIRS);
    cards = shuffle([...chosen, ...chosen]).map((icon) => ({ icon, flipped: false, matched: false }));
    firstPick = null;
    secondPick = null;
    moves = 0;
    matches = 0;
    locked = false;
    score = 0;
    won = false;
    startTime = Date.now();
    dispatch('score', score);
  }

  function pick(i: number) {
    if (locked || cards[i].flipped || cards[i].matched || won) return;
    cards[i].flipped = true;
    cards = cards;

    if (firstPick === null) {
      firstPick = i;
      return;
    }
    secondPick = i;
    moves++;
    locked = true;

    if (cards[firstPick].icon === cards[secondPick].icon) {
      cards[firstPick].matched = true;
      cards[secondPick].matched = true;
      cards = cards;
      matches++;
      // Fewer moves = higher score per match; reward efficiency.
      score += Math.max(50, 200 - moves * 3);
      dispatch('score', score);
      firstPick = null;
      secondPick = null;
      locked = false;
      if (matches === PAIRS) {
        won = true;
        const elapsedSec = Math.round((Date.now() - startTime) / 1000);
        score += Math.max(0, 500 - elapsedSec * 5); // time bonus
        dispatch('score', score);
        dispatch('gameOver', score);
      }
    } else {
      setTimeout(() => {
        if (firstPick !== null) cards[firstPick].flipped = false;
        if (secondPick !== null) cards[secondPick].flipped = false;
        cards = cards;
        firstPick = null;
        secondPick = null;
        locked = false;
      }, 700);
    }
  }

  onMount(reset);
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    <span>Moves: {moves}</span>
    <span>{matches}/{PAIRS} matched</span>
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <div class="grid grid-cols-4 gap-2">
    {#each cards as card, i (i)}
      <button on:click={() => pick(i)}
        class="w-16 h-16 rounded-xl flex items-center justify-center text-2xl transition-all duration-200 {card.flipped || card.matched ? 'bg-slate-700' : 'bg-slate-800 hover:bg-slate-750'} {card.matched ? 'opacity-50 ring-2 ring-emerald-400' : ''}">
        {#if card.flipped || card.matched}{card.icon}{:else}<span class="text-slate-600">?</span>{/if}
      </button>
    {/each}
  </div>
  {#if won}
    <p class="text-emerald-400 text-sm font-semibold">Solved in {moves} moves! 🎉</p>
  {/if}
  <p class="text-xs text-slate-500">Click two cards to find a matching pair.</p>
</div>
