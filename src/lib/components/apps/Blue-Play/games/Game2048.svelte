<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const SIZE = 4;
  let board: number[][] = [];
  let score = 0;
  let over = false;
  let won = false;

  const TILE_COLORS: Record<number, string> = {
    2: 'bg-slate-700 text-slate-100', 4: 'bg-slate-600 text-slate-100',
    8: 'bg-amber-700 text-white', 16: 'bg-amber-600 text-white',
    32: 'bg-orange-600 text-white', 64: 'bg-orange-500 text-white',
    128: 'bg-yellow-500 text-white', 256: 'bg-yellow-400 text-slate-900',
    512: 'bg-yellow-300 text-slate-900', 1024: 'bg-yellow-200 text-slate-900',
    2048: 'bg-emerald-400 text-slate-900',
  };

  function emptyBoard(): number[][] {
    return Array.from({ length: SIZE }, () => Array(SIZE).fill(0));
  }

  function emptyCells(b: number[][]): [number, number][] {
    const cells: [number, number][] = [];
    for (let r = 0; r < SIZE; r++) for (let c = 0; c < SIZE; c++) if (b[r][c] === 0) cells.push([r, c]);
    return cells;
  }

  function addRandomTile(b: number[][]) {
    const cells = emptyCells(b);
    if (cells.length === 0) return;
    const [r, c] = cells[Math.floor(Math.random() * cells.length)];
    b[r][c] = Math.random() < 0.9 ? 2 : 4;
  }

  function reset() {
    board = emptyBoard();
    addRandomTile(board);
    addRandomTile(board);
    score = 0;
    over = false;
    won = false;
    dispatch('score', score);
  }

  function slideRowLeft(row: number[]): { row: number[]; gained: number } {
    let vals = row.filter((v) => v !== 0);
    let gained = 0;
    for (let i = 0; i < vals.length - 1; i++) {
      if (vals[i] === vals[i + 1]) {
        vals[i] *= 2;
        gained += vals[i];
        if (vals[i] === 2048) won = true;
        vals.splice(i + 1, 1);
      }
    }
    while (vals.length < SIZE) vals.push(0);
    return { row: vals, gained };
  }

  function rotateLeft(b: number[][]): number[][] {
    const n = emptyBoard();
    for (let r = 0; r < SIZE; r++) for (let c = 0; c < SIZE; c++) n[SIZE - 1 - c][r] = b[r][c];
    return n;
  }

  function move(direction: 'up' | 'down' | 'left' | 'right') {
    if (over) return;
    let rotations = { left: 0, up: 1, right: 2, down: 3 }[direction];
    let b = board.map((r) => [...r]);
    for (let i = 0; i < rotations; i++) b = rotateLeft(b);

    let moved = false;
    let gainedTotal = 0;
    const newB = b.map((row) => {
      const before = row.join(',');
      const { row: slid, gained } = slideRowLeft(row);
      gainedTotal += gained;
      if (slid.join(',') !== before) moved = true;
      return slid;
    });

    let result = newB;
    for (let i = 0; i < (4 - rotations) % 4; i++) result = rotateLeft(result);

    if (!moved) return;
    board = result;
    score += gainedTotal;
    dispatch('score', score);
    addRandomTile(board);

    if (emptyCells(board).length === 0 && !canMove(board)) {
      over = true;
      dispatch('gameOver', score);
    }
  }

  function canMove(b: number[][]): boolean {
    for (let r = 0; r < SIZE; r++) {
      for (let c = 0; c < SIZE; c++) {
        const v = b[r][c];
        if (v === 0) return true;
        if (c < SIZE - 1 && b[r][c + 1] === v) return true;
        if (r < SIZE - 1 && b[r + 1][c] === v) return true;
      }
    }
    return false;
  }

  function handleKey(e: KeyboardEvent) {
    const map: Record<string, 'up' | 'down' | 'left' | 'right'> = {
      ArrowUp: 'up', w: 'up', W: 'up',
      ArrowDown: 'down', s: 'down', S: 'down',
      ArrowLeft: 'left', a: 'left', A: 'left',
      ArrowRight: 'right', d: 'right', D: 'right',
    };
    if (map[e.key]) { e.preventDefault(); move(map[e.key]); }
  }

  // Basic touch swipe support so this isn't keyboard-only.
  let touchStart: { x: number; y: number } | null = null;
  function onTouchStart(e: TouchEvent) { touchStart = { x: e.touches[0].clientX, y: e.touches[0].clientY }; }
  function onTouchEnd(e: TouchEvent) {
    if (!touchStart) return;
    const dx = e.changedTouches[0].clientX - touchStart.x;
    const dy = e.changedTouches[0].clientY - touchStart.y;
    if (Math.max(Math.abs(dx), Math.abs(dy)) > 30) {
      if (Math.abs(dx) > Math.abs(dy)) move(dx > 0 ? 'right' : 'left');
      else move(dy > 0 ? 'down' : 'up');
    }
    touchStart = null;
  }

  onMount(() => {
    reset();
    window.addEventListener('keydown', handleKey);
  });
  onDestroy(() => window.removeEventListener('keydown', handleKey));
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    {#if won}<span class="text-emerald-400 text-xs font-semibold">2048 reached! Keep going.</span>{/if}
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <div class="relative bg-slate-800 p-2 rounded-xl border border-white/10"
    on:touchstart={onTouchStart} on:touchend={onTouchEnd}>
    <div class="grid grid-cols-4 gap-2" style="width: 272px;">
      {#each board as row, r (r)}
        {#each row as cell, c (c)}
          <div class="w-16 h-16 rounded-lg flex items-center justify-center font-bold text-lg transition-colors {cell === 0 ? 'bg-slate-900/60' : (TILE_COLORS[cell] || 'bg-emerald-300 text-slate-900')}">
            {cell !== 0 ? cell : ''}
          </div>
        {/each}
      {/each}
    </div>
    {#if over}
      <div class="absolute inset-0 bg-slate-950/80 rounded-xl flex flex-col items-center justify-center gap-2">
        <span class="text-white font-bold">Game Over</span>
        <button on:click={reset} class="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm">Try again</button>
      </div>
    {/if}
  </div>
  <p class="text-xs text-slate-500">Arrow keys / WASD / swipe to slide tiles.</p>
</div>
