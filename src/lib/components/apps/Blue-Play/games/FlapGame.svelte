<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const W = 360, H = 480;
  const GRAVITY = 0.45, FLAP_V = -7.5, BIRD_R = 10;
  const PIPE_W = 52, GAP = 130, PIPE_SPEED = 2.4, PIPE_INTERVAL = 105;

  interface Pipe { x: number; gapY: number; passed: boolean; }

  let canvasEl: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let raf: number;

  let birdY = H / 2, birdV = 0;
  let pipes: Pipe[] = [];
  let frame = 0;
  let score = 0;
  let over = false;
  let started = false;

  function reset() {
    birdY = H / 2; birdV = 0;
    pipes = [];
    frame = 0;
    score = 0;
    over = false;
    started = false;
    dispatch('score', score);
  }

  function flap() {
    if (over) { reset(); return; }
    started = true;
    birdV = FLAP_V;
  }

  function step() {
    if (started && !over) {
      frame++;
      birdV += GRAVITY;
      birdY += birdV;

      if (frame % PIPE_INTERVAL === 0) {
        const gapY = 60 + Math.random() * (H - 120 - GAP);
        pipes.push({ x: W, gapY, passed: false });
      }
      pipes = pipes.map((p) => ({ ...p, x: p.x - PIPE_SPEED })).filter((p) => p.x > -PIPE_W);

      for (const p of pipes) {
        if (!p.passed && p.x + PIPE_W < W / 2 - BIRD_R) {
          p.passed = true;
          score++;
          dispatch('score', score);
        }
        const birdX = W / 2;
        const hitsPipeX = birdX + BIRD_R > p.x && birdX - BIRD_R < p.x + PIPE_W;
        const hitsPipeY = birdY - BIRD_R < p.gapY || birdY + BIRD_R > p.gapY + GAP;
        if (hitsPipeX && hitsPipeY) { over = true; dispatch('gameOver', score); }
      }

      if (birdY + BIRD_R > H || birdY - BIRD_R < 0) { over = true; dispatch('gameOver', score); }
    }
    draw();
    raf = requestAnimationFrame(step);
  }

  function draw() {
    if (!ctx) return;
    const grad = ctx.createLinearGradient(0, 0, 0, H);
    grad.addColorStop(0, '#1e3a5f'); grad.addColorStop(1, '#0f172a');
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, W, H);

    ctx.fillStyle = '#4ade80';
    for (const p of pipes) {
      ctx.fillRect(p.x, 0, PIPE_W, p.gapY);
      ctx.fillRect(p.x, p.gapY + GAP, PIPE_W, H - p.gapY - GAP);
    }

    ctx.save();
    ctx.translate(W / 2, birdY);
    ctx.rotate(Math.max(-0.5, Math.min(0.9, birdV * 0.06)));
    ctx.fillStyle = '#fbbf24';
    ctx.beginPath(); ctx.arc(0, 0, BIRD_R, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#0f172a';
    ctx.beginPath(); ctx.arc(4, -3, 2, 0, Math.PI * 2); ctx.fill();
    ctx.restore();

    ctx.fillStyle = '#fff';
    ctx.font = 'bold 28px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(String(score), W / 2, 50);

    if (!started) {
      ctx.font = '14px sans-serif';
      ctx.fillText('Click or press Space to start', W / 2, H / 2);
    }
    if (over) {
      ctx.fillStyle = 'rgba(15,23,42,0.8)';
      ctx.fillRect(0, 0, W, H);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 22px sans-serif';
      ctx.fillText('Game Over', W / 2, H / 2 - 10);
      ctx.font = '13px sans-serif';
      ctx.fillText('Click or press Space to retry', W / 2, H / 2 + 16);
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === ' ') { e.preventDefault(); flap(); }
  }

  onMount(() => {
    ctx = canvasEl.getContext('2d')!;
    reset();
    draw();
    raf = requestAnimationFrame(step);
    window.addEventListener('keydown', handleKey);
  });
  onDestroy(() => {
    cancelAnimationFrame(raf);
    window.removeEventListener('keydown', handleKey);
  });
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <canvas bind:this={canvasEl} width={W} height={H} class="rounded-xl border border-white/10 cursor-pointer" on:click={flap} />
  <p class="text-xs text-slate-500">Click the game or press Space to flap.</p>
</div>
