<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { Flag, Bomb, RotateCcw } from 'lucide-svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const ROWS = 12, COLS = 12, MINES = 20;

  interface Cell { mine: boolean; revealed: boolean; flagged: boolean; adjacent: number; }

  let grid: Cell[][] = [];
  let over = false;
  let won = false;
  let flagsUsed = 0;
  let revealedCount = 0;
  let startTime = 0;
  let score = 0;

  function emptyGrid(): Cell[][] {
    return Array.from({ length: ROWS }, () => Array.from({ length: COLS }, () => ({
      mine: false, revealed: false, flagged: false, adjacent: 0,
    })));
  }

  function placeMines(avoidR: number, avoidC: number) {
    let placed = 0;
    while (placed < MINES) {
      const r = Math.floor(Math.random() * ROWS);
      const c = Math.floor(Math.random() * COLS);
      if (grid[r][c].mine) continue;
      if (Math.abs(r - avoidR) <= 1 && Math.abs(c - avoidC) <= 1) continue;
      grid[r][c].mine = true;
      placed++;
    }
    for (let r = 0; r < ROWS; r++) {
      for (let c = 0; c < COLS; c++) {
        if (grid[r][c].mine) continue;
        let count = 0;
        for (let dr = -1; dr <= 1; dr++) for (let dc = -1; dc <= 1; dc++) {
          const nr = r + dr, nc = c + dc;
          if (nr >= 0 && nr < ROWS && nc >= 0 && nc < COLS && grid[nr][nc].mine) count++;
        }
        grid[r][c].adjacent = count;
      }
    }
  }

  let firstClick = true;

  function reset() {
    grid = emptyGrid();
    over = false; won = false; flagsUsed = 0; revealedCount = 0; score = 0;
    firstClick = true;
    startTime = Date.now();
    dispatch('score', score);
  }

  function reveal(r: number, c: number) {
    if (over || won) return;
    if (grid[r][c].flagged || grid[r][c].revealed) return;

    if (firstClick) {
      placeMines(r, c);
      firstClick = false;
    }

    const stack: [number, number][] = [[r, c]];
    while (stack.length) {
      const [cr, cc] = stack.pop()!;
      const cell = grid[cr][cc];
      if (cell.revealed || cell.flagged) continue;
      cell.revealed = true;
      revealedCount++;
      if (cell.mine) {
        over = true;
        for (const row of grid) for (const cell2 of row) if (cell2.mine) cell2.revealed = true;
        dispatch('gameOver', score);
        grid = grid;
        return;
      }
      if (cell.adjacent === 0) {
        for (let dr = -1; dr <= 1; dr++) for (let dc = -1; dc <= 1; dc++) {
          const nr = cr + dr, nc = cc + dc;
          if (nr >= 0 && nr < ROWS && nc >= 0 && nc < COLS && !grid[nr][nc].revealed) stack.push([nr, nc]);
        }
      }
    }
    grid = grid;

    if (revealedCount === ROWS * COLS - MINES) {
      won = true;
      const elapsed = Math.round((Date.now() - startTime) / 1000);
      score = Math.max(100, 2000 - elapsed * 10);
      dispatch('score', score);
      dispatch('gameOver', score);
    }
  }

  function toggleFlag(r: number, c: number, e: MouseEvent) {
    e.preventDefault();
    if (over || won || grid[r][c].revealed) return;
    grid[r][c].flagged = !grid[r][c].flagged;
    flagsUsed += grid[r][c].flagged ? 1 : -1;
    grid = grid;
  }

  const NUM_COLORS = ['', '#60a5fa', '#4ade80', '#f87171', '#a78bfa', '#fbbf24', '#22d3ee', '#f8fafc', '#94a3b8'];

  onMount(reset);
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span class="flex items-center gap-1"><Bomb size={13} class="text-red-400" /> {MINES - flagsUsed}</span>
    <button on:click={reset} class="flex items-center gap-1.5 px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs"><RotateCcw size={11} /> Restart</button>
    {#if won}<span class="text-emerald-400 font-semibold">Cleared! Score: {score}</span>{/if}
    {#if over}<span class="text-red-400 font-semibold">Boom!</span>{/if}
  </div>
  <div class="grid gap-[1px] bg-slate-800 p-1 rounded-lg" style="grid-template-columns: repeat({COLS}, 22px);">
    {#each grid as row, r (r)}
      {#each row as cell, c (c)}
        <button
          on:click={() => reveal(r, c)}
          on:contextmenu={(e) => toggleFlag(r, c, e)}
          class="w-[22px] h-[22px] flex items-center justify-center text-[11px] font-bold select-none
            {cell.revealed ? (cell.mine ? 'bg-red-900' : 'bg-slate-900') : 'bg-slate-700 hover:bg-slate-650'}"
          style={cell.revealed && cell.adjacent > 0 ? `color:${NUM_COLORS[cell.adjacent]}` : ''}
        >
          {#if cell.revealed}
            {#if cell.mine}💣{:else if cell.adjacent > 0}{cell.adjacent}{/if}
          {:else if cell.flagged}
            <Flag size={11} class="text-amber-400" />
          {/if}
        </button>
      {/each}
    {/each}
  </div>
  <p class="text-xs text-slate-500">Left click to reveal, right click to flag.</p>
</div>
