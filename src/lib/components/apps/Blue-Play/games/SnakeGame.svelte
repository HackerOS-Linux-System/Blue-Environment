<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const GRID = 20;
  const CELL = 20; // px
  const TICK_MS = 110;

  type Point = { x: number; y: number };

  let canvasEl: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let snake: Point[] = [{ x: 10, y: 10 }, { x: 9, y: 10 }, { x: 8, y: 10 }];
  let dir: Point = { x: 1, y: 0 };
  let nextDir: Point = { x: 1, y: 0 };
  let food: Point = { x: 15, y: 10 };
  let score = 0;
  let paused = false;
  let over = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  function randomFood(): Point {
    let p: Point;
    do {
      p = { x: Math.floor(Math.random() * GRID), y: Math.floor(Math.random() * GRID) };
    } while (snake.some((s) => s.x === p.x && s.y === p.y));
    return p;
  }

  function reset() {
    snake = [{ x: 10, y: 10 }, { x: 9, y: 10 }, { x: 8, y: 10 }];
    dir = { x: 1, y: 0 };
    nextDir = { x: 1, y: 0 };
    food = randomFood();
    score = 0;
    over = false;
    paused = false;
    dispatch('score', score);
  }

  function tick() {
    if (paused || over) return;
    dir = nextDir;
    const head = { x: snake[0].x + dir.x, y: snake[0].y + dir.y };

    const hitWall = head.x < 0 || head.y < 0 || head.x >= GRID || head.y >= GRID;
    const hitSelf = snake.some((s) => s.x === head.x && s.y === head.y);
    if (hitWall || hitSelf) {
      over = true;
      dispatch('gameOver', score);
      draw();
      return;
    }

    snake = [head, ...snake];
    if (head.x === food.x && head.y === food.y) {
      score += 10;
      dispatch('score', score);
      food = randomFood();
    } else {
      snake.pop();
    }
    draw();
  }

  function draw() {
    if (!ctx) return;
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, GRID * CELL, GRID * CELL);

    ctx.fillStyle = '#f59e0b';
    ctx.fillRect(food.x * CELL + 2, food.y * CELL + 2, CELL - 4, CELL - 4);

    snake.forEach((s, i) => {
      ctx.fillStyle = i === 0 ? '#34d399' : '#10b981';
      ctx.fillRect(s.x * CELL + 1, s.y * CELL + 1, CELL - 2, CELL - 2);
    });

    if (over) {
      ctx.fillStyle = 'rgba(15,23,42,0.75)';
      ctx.fillRect(0, 0, GRID * CELL, GRID * CELL);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 20px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Game Over', GRID * CELL / 2, GRID * CELL / 2 - 10);
      ctx.font = '13px sans-serif';
      ctx.fillText('Press Space to retry', GRID * CELL / 2, GRID * CELL / 2 + 14);
    } else if (paused) {
      ctx.fillStyle = 'rgba(15,23,42,0.6)';
      ctx.fillRect(0, 0, GRID * CELL, GRID * CELL);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 18px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Paused', GRID * CELL / 2, GRID * CELL / 2);
    }
  }

  function setDir(x: number, y: number) {
    // Disallow reversing directly into yourself
    if (snake.length > 1 && dir.x === -x && dir.y === -y) return;
    nextDir = { x, y };
  }

  function handleKey(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowUp': case 'w': case 'W': setDir(0, -1); e.preventDefault(); break;
      case 'ArrowDown': case 's': case 'S': setDir(0, 1); e.preventDefault(); break;
      case 'ArrowLeft': case 'a': case 'A': setDir(-1, 0); e.preventDefault(); break;
      case 'ArrowRight': case 'd': case 'D': setDir(1, 0); e.preventDefault(); break;
      case ' ':
        e.preventDefault();
        if (over) reset(); else paused = !paused;
        draw();
        break;
    }
  }

  onMount(() => {
    ctx = canvasEl.getContext('2d')!;
    draw();
    timer = setInterval(tick, TICK_MS);
    window.addEventListener('keydown', handleKey);
  });
  onDestroy(() => {
    clearInterval(timer);
    window.removeEventListener('keydown', handleKey);
  });
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    <button on:click={() => { paused = !paused; draw(); }} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">
      {paused ? 'Resume' : 'Pause'}
    </button>
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <canvas bind:this={canvasEl} width={GRID * CELL} height={GRID * CELL} class="rounded-xl border border-white/10" />
  <p class="text-xs text-slate-500">Arrow keys / WASD to steer. Space to pause or retry.</p>
</div>
