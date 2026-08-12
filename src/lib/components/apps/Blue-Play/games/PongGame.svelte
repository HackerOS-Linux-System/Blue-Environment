<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const W = 480, H = 320, PADDLE_W = 10, PADDLE_H = 60, BALL_SIZE = 8;
  const WIN_SCORE = 7;

  let canvasEl: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let raf: number;

  let playerY = H / 2 - PADDLE_H / 2;
  let aiY = H / 2 - PADDLE_H / 2;
  let ball = { x: W / 2, y: H / 2, vx: 4, vy: 3 };
  let playerScore = 0;
  let aiScore = 0;
  let over = false;
  let paused = false;

  function resetBall(direction: number) {
    ball = { x: W / 2, y: H / 2, vx: 4 * direction, vy: (Math.random() - 0.5) * 6 };
  }

  function reset() {
    playerScore = 0; aiScore = 0; over = false; paused = false;
    playerY = H / 2 - PADDLE_H / 2;
    aiY = H / 2 - PADDLE_H / 2;
    resetBall(Math.random() < 0.5 ? 1 : -1);
    dispatch('score', playerScore);
  }

  function step() {
    if (!paused && !over) {
      ball.x += ball.vx;
      ball.y += ball.vy;

      if (ball.y <= BALL_SIZE / 2 || ball.y >= H - BALL_SIZE / 2) ball.vy *= -1;

      // Player paddle (left)
      if (ball.x <= PADDLE_W + BALL_SIZE / 2 && ball.x > PADDLE_W - 4 &&
          ball.y >= playerY && ball.y <= playerY + PADDLE_H && ball.vx < 0) {
        const hit = (ball.y - playerY) / PADDLE_H - 0.5;
        ball.vx = Math.abs(ball.vx) * 1.05;
        ball.vy = hit * 8;
      }
      // AI paddle (right) — simple tracking with imperfect speed, not omniscient
      const aiCenter = aiY + PADDLE_H / 2;
      if (aiCenter < ball.y - 10) aiY += 3.3;
      else if (aiCenter > ball.y + 10) aiY -= 3.3;
      aiY = Math.max(0, Math.min(H - PADDLE_H, aiY));

      if (ball.x >= W - PADDLE_W - BALL_SIZE / 2 && ball.x < W - PADDLE_W + 4 &&
          ball.y >= aiY && ball.y <= aiY + PADDLE_H && ball.vx > 0) {
        const hit = (ball.y - aiY) / PADDLE_H - 0.5;
        ball.vx = -Math.abs(ball.vx) * 1.05;
        ball.vy = hit * 8;
      }

      if (ball.x < 0) {
        aiScore++;
        if (aiScore >= WIN_SCORE) { over = true; dispatch('gameOver', playerScore); }
        else resetBall(1);
      } else if (ball.x > W) {
        playerScore++;
        dispatch('score', playerScore);
        if (playerScore >= WIN_SCORE) { over = true; dispatch('gameOver', playerScore); }
        else resetBall(-1);
      }
    }
    draw();
    raf = requestAnimationFrame(step);
  }

  function draw() {
    if (!ctx) return;
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, W, H);
    ctx.strokeStyle = '#1e293b';
    ctx.setLineDash([6, 8]);
    ctx.beginPath(); ctx.moveTo(W / 2, 0); ctx.lineTo(W / 2, H); ctx.stroke();
    ctx.setLineDash([]);

    ctx.fillStyle = '#60a5fa';
    ctx.fillRect(0, playerY, PADDLE_W, PADDLE_H);
    ctx.fillStyle = '#f87171';
    ctx.fillRect(W - PADDLE_W, aiY, PADDLE_W, PADDLE_H);
    ctx.fillStyle = '#f8fafc';
    ctx.fillRect(ball.x - BALL_SIZE / 2, ball.y - BALL_SIZE / 2, BALL_SIZE, BALL_SIZE);

    ctx.font = 'bold 24px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillStyle = '#60a5fa';
    ctx.fillText(String(playerScore), W / 2 - 40, 32);
    ctx.fillStyle = '#f87171';
    ctx.fillText(String(aiScore), W / 2 + 40, 32);

    if (over) {
      ctx.fillStyle = 'rgba(15,23,42,0.85)';
      ctx.fillRect(0, 0, W, H);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 20px sans-serif';
      ctx.fillText(playerScore > aiScore ? 'You win!' : 'AI wins', W / 2, H / 2 - 8);
      ctx.font = '13px sans-serif';
      ctx.fillText('Press Space to play again', W / 2, H / 2 + 16);
    } else if (paused) {
      ctx.fillStyle = 'rgba(15,23,42,0.6)';
      ctx.fillRect(0, 0, W, H);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 18px sans-serif';
      ctx.fillText('Paused', W / 2, H / 2);
    }
  }

  function handlePointerMove(e: PointerEvent) {
    const rect = canvasEl.getBoundingClientRect();
    playerY = Math.max(0, Math.min(H - PADDLE_H, e.clientY - rect.top - PADDLE_H / 2));
  }
  function handleKey(e: KeyboardEvent) {
    if (e.key === ' ') { e.preventDefault(); if (over) reset(); else paused = !paused; }
    if (e.key === 'ArrowUp' || e.key === 'w' || e.key === 'W') playerY = Math.max(0, playerY - 20);
    if (e.key === 'ArrowDown' || e.key === 's' || e.key === 'S') playerY = Math.min(H - PADDLE_H, playerY + 20);
  }

  onMount(() => {
    ctx = canvasEl.getContext('2d')!;
    reset();
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
    <span>You <strong class="text-blue-400">{playerScore}</strong> — <strong class="text-red-400">{aiScore}</strong> AI</span>
    <button on:click={() => (paused = !paused)} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">{paused ? 'Resume' : 'Pause'}</button>
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <canvas bind:this={canvasEl} width={W} height={H} class="rounded-xl border border-white/10 cursor-none"
    on:pointermove={handlePointerMove} />
  <p class="text-xs text-slate-500">Mouse or ↑/↓ / W/S to move. First to {WIN_SCORE} wins.</p>
</div>
