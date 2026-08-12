<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const ROWS = 6, COLS = 7;
  type Cell = 0 | 1 | 2; // 0 empty, 1 red, 2 yellow

  let board: Cell[][] = [];
  let current: 1 | 2 = 1;
  let winner: 0 | 1 | 2 = 0;
  let draw = false;
  let winningCells: [number, number][] = [];
  let wins = { 1: 0, 2: 0 };
  let moveCount = 0;

  function emptyBoard(): Cell[][] {
    return Array.from({ length: ROWS }, () => Array(COLS).fill(0)) as Cell[][];
  }

  function reset() {
    board = emptyBoard();
    current = 1;
    winner = 0;
    draw = false;
    winningCells = [];
    moveCount = 0;
    dispatch('score', wins[1] + wins[2]);
  }

  function lowestEmptyRow(col: number): number {
    for (let r = ROWS - 1; r >= 0; r--) if (board[r][col] === 0) return r;
    return -1;
  }

  function checkWin(r: number, c: number, player: Cell): [number, number][] | null {
    const dirs = [[0, 1], [1, 0], [1, 1], [1, -1]];
    for (const [dr, dc] of dirs) {
      const line: [number, number][] = [[r, c]];
      for (const sign of [1, -1]) {
        let nr = r + dr * sign, nc = c + dc * sign;
        while (nr >= 0 && nr < ROWS && nc >= 0 && nc < COLS && board[nr][nc] === player) {
          line.push([nr, nc]);
          nr += dr * sign; nc += dc * sign;
        }
      }
      if (line.length >= 4) return line;
    }
    return null;
  }

  function drop(col: number) {
    if (winner || draw) return;
    const row = lowestEmptyRow(col);
    if (row === -1) return;
    board[row][col] = current;
    board = board;
    moveCount++;

    const win = checkWin(row, col, current);
    if (win) {
      winner = current;
      winningCells = win;
      wins[current]++;
      dispatch('gameOver', wins[1] + wins[2]);
      dispatch('score', wins[1] + wins[2]);
      return;
    }
    if (moveCount === ROWS * COLS) {
      draw = true;
      dispatch('gameOver', wins[1] + wins[2]);
      return;
    }
    current = current === 1 ? 2 : 1;
  }

  function isWinCell(r: number, c: number): boolean {
    return winningCells.some(([wr, wc]) => wr === r && wc === c);
  }

  onMount(reset);
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-full bg-red-500 inline-block" /> {wins[1]}</span>
    <span class="flex items-center gap-1.5"><span class="w-3 h-3 rounded-full bg-yellow-400 inline-block" /> {wins[2]}</span>
    {#if !winner && !draw}
      <span>Turn: <strong class={current === 1 ? 'text-red-400' : 'text-yellow-300'}>{current === 1 ? 'Red' : 'Yellow'}</strong></span>
    {/if}
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">New round</button>
  </div>

  <div class="bg-blue-800 p-2 rounded-2xl">
    <div class="grid gap-1" style="grid-template-columns: repeat({COLS}, 40px);">
      {#each Array(COLS) as _, c (c)}
        <button on:click={() => drop(c)} class="h-6 flex items-center justify-center text-blue-300 hover:text-white text-xs">▼</button>
      {/each}
      {#each board as row, r (r)}
        {#each row as cell, c (c)}
          <div class="w-10 h-10 rounded-full flex items-center justify-center {isWinCell(r, c) ? 'ring-2 ring-white' : ''}"
            style="background:#1e3a8a;">
            {#if cell !== 0}
              <span class="w-8 h-8 rounded-full block {cell === 1 ? 'bg-red-500' : 'bg-yellow-400'}" />
            {/if}
          </div>
        {/each}
      {/each}
    </div>
  </div>

  {#if winner}
    <p class="text-sm font-semibold {winner === 1 ? 'text-red-400' : 'text-yellow-300'}">{winner === 1 ? 'Red' : 'Yellow'} wins!</p>
  {:else if draw}
    <p class="text-sm text-slate-400 font-semibold">Draw!</p>
  {/if}
  <p class="text-xs text-slate-500">Local 2-player — click a column to drop a piece.</p>
</div>
