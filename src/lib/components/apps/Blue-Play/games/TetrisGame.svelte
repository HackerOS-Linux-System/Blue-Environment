<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const COLS = 10, ROWS = 20, CELL = 18;
  const DROP_MS = 550;

  type Board = number[][]; // 0 = empty, 1..7 = piece color index

  const COLORS = ['', '#22d3ee', '#fbbf24', '#a78bfa', '#4ade80', '#f87171', '#60a5fa', '#fb923c'];

  // Standard tetromino shapes (rotation state 0), each a set of [row,col] offsets.
  const SHAPES: number[][][] = [
    [[0,0],[0,1],[1,0],[1,1]],          // O
    [[0,0],[0,1],[0,2],[0,3]],          // I
    [[0,0],[1,0],[1,1],[1,2]],          // J
    [[0,2],[1,0],[1,1],[1,2]],          // L
    [[0,1],[0,2],[1,0],[1,1]],          // S
    [[0,0],[0,1],[1,1],[1,2]],          // Z
    [[0,1],[1,0],[1,1],[1,2]],          // T
  ];

  let board: Board = [];
  let cur: { shape: number[][]; color: number; row: number; col: number } | null = null;
  let score = 0;
  let lines = 0;
  let over = false;
  let paused = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  function emptyBoard(): Board {
    return Array.from({ length: ROWS }, () => Array(COLS).fill(0));
  }

  function spawnPiece() {
    const idx = Math.floor(Math.random() * SHAPES.length);
    const shape = SHAPES[idx].map(([r, c]) => [r, c]);
    cur = { shape, color: idx + 1, row: 0, col: Math.floor(COLS / 2) - 1 };
    if (collides(cur.shape, cur.row, cur.col)) {
      over = true;
      dispatch('gameOver', score);
    }
  }

  function collides(shape: number[][], row: number, col: number): boolean {
    return shape.some(([r, c]) => {
      const br = row + r, bc = col + c;
      if (bc < 0 || bc >= COLS || br >= ROWS) return true;
      if (br < 0) return false;
      return board[br][bc] !== 0;
    });
  }

  function rotate(shape: number[][]): number[][] {
    // Rotate around the shape's own bounding box (simple, works fine for
    // a casual implementation even if not perfectly SRS-accurate).
    const maxR = Math.max(...shape.map(([r]) => r));
    return shape.map(([r, c]) => [c, maxR - r]);
  }

  function lockPiece() {
    if (!cur) return;
    for (const [r, c] of cur.shape) {
      const br = cur.row + r, bc = cur.col + c;
      if (br >= 0) board[br][bc] = cur.color;
    }
    clearLines();
    spawnPiece();
  }

  function clearLines() {
    let cleared = 0;
    board = board.filter((row) => {
      const full = row.every((cell) => cell !== 0);
      if (full) cleared++;
      return !full;
    });
    while (board.length < ROWS) board.unshift(Array(COLS).fill(0));
    if (cleared > 0) {
      lines += cleared;
      score += [0, 100, 300, 500, 800][cleared] * Math.max(1, Math.floor(lines / 10));
      dispatch('score', score);
    }
  }

  function tryMove(dr: number, dc: number): boolean {
    if (!cur || over || paused) return false;
    const nr = cur.row + dr, nc = cur.col + dc;
    if (collides(cur.shape, nr, nc)) return false;
    cur.row = nr; cur.col = nc;
    return true;
  }

  function tryRotate() {
    if (!cur || over || paused) return;
    const rotated = rotate(cur.shape);
    if (!collides(rotated, cur.row, cur.col)) cur.shape = rotated;
  }

  function hardDrop() {
    if (!cur || over || paused) return;
    while (tryMove(1, 0)) { /* keep falling */ }
    lockPiece();
  }

  function tick() {
    if (over || paused || !cur) return;
    if (!tryMove(1, 0)) lockPiece();
  }

  function reset() {
    board = emptyBoard();
    score = 0;
    lines = 0;
    over = false;
    paused = false;
    spawnPiece();
    dispatch('score', score);
  }

  function handleKey(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowLeft': case 'a': case 'A': tryMove(0, -1); e.preventDefault(); break;
      case 'ArrowRight': case 'd': case 'D': tryMove(0, 1); e.preventDefault(); break;
      case 'ArrowDown': case 's': case 'S': tryMove(1, 0); e.preventDefault(); break;
      case 'ArrowUp': case 'w': case 'W': tryRotate(); e.preventDefault(); break;
      case ' ': e.preventDefault(); if (over) reset(); else hardDrop(); break;
      case 'p': case 'P': paused = !paused; break;
    }
  }

  onMount(() => {
    reset();
    timer = setInterval(tick, DROP_MS);
    window.addEventListener('keydown', handleKey);
  });
  onDestroy(() => {
    clearInterval(timer);
    window.removeEventListener('keydown', handleKey);
  });

  $: displayBoard = (() => {
    const b = board.map((row) => [...row]);
    if (cur) for (const [r, c] of cur.shape) {
      const br = cur.row + r, bc = cur.col + c;
      if (br >= 0 && br < ROWS && bc >= 0 && bc < COLS) b[br][bc] = cur.color;
    }
    return b;
  })();
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    <span>Lines: {lines}</span>
    <button on:click={() => (paused = !paused)} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">{paused ? 'Resume' : 'Pause'}</button>
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <div class="relative bg-slate-950 rounded-xl border border-white/10 p-1" style="width:{COLS * CELL + 8}px;">
    <div class="grid" style="grid-template-columns: repeat({COLS}, {CELL}px);">
      {#each displayBoard as row, r (r)}
        {#each row as cell, c (c)}
          <div class="border border-slate-900/40" style="width:{CELL}px; height:{CELL}px; background:{cell ? COLORS[cell] : 'transparent'};" />
        {/each}
      {/each}
    </div>
    {#if over || paused}
      <div class="absolute inset-0 bg-slate-950/80 flex items-center justify-center">
        <span class="text-white font-bold">{over ? 'Game Over' : 'Paused'}</span>
      </div>
    {/if}
  </div>
  <p class="text-xs text-slate-500">←/→ move, ↑ rotate, ↓ soft drop, Space hard drop/retry, P pause.</p>
</div>
