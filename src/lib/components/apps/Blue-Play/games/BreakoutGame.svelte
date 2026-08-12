<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';

  const dispatch = createEventDispatcher<{ score: number; gameOver: number }>();

  const W = 400, H = 400;
  const PADDLE_W = 70, PADDLE_H = 10;
  const BALL_R = 6;
  const BRICK_ROWS = 5, BRICK_COLS = 8;
  const BRICK_W = W / BRICK_COLS, BRICK_H = 18;
  const BRICK_COLORS = ['#f87171', '#fb923c', '#facc15', '#4ade80', '#60a5fa'];

  let canvasEl: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let raf: number;

  let paddleX = W / 2 - PADDLE_W / 2;
  let ball = { x: W / 2, y: H - 40, vx: 3, vy: -3 };
  let bricks: boolean[][] = [];
  let score = 0;
  let lives = 3;
  let over = false;
  let won = false;
  let paused = false;
  let leftHeld = false, rightHeld = false;

  function resetBricks() {
    bricks = Array.from({ length: BRICK_ROWS }, () => Array(BRICK_COLS).fill(true));
  }

  function resetBall() {
    ball = { x: W / 2, y: H - 40, vx: 3 * (Math.random() < 0.5 ? 1 : -1), vy: -3 };
    paddleX = W / 2 - PADDLE_W / 2;
  }

  function reset() {
    resetBricks();
    resetBall();
    score = 0;
    lives = 3;
    over = false;
    won = false;
    paused = false;
    dispatch('score', score);
  }

  function step() {
    if (!paused && !over) {
      if (leftHeld) paddleX -= 6;
      if (rightHeld) paddleX += 6;
      paddleX = Math.max(0, Math.min(W - PADDLE_W, paddleX));

      ball.x += ball.vx;
      ball.y += ball.vy;

      if (ball.x <= BALL_R || ball.x >= W - BALL_R) ball.vx *= -1;
      if (ball.y <= BALL_R) ball.vy *= -1;

      // Paddle collision
      if (ball.y >= H - 20 - BALL_R && ball.y <= H - 10 &&
          ball.x >= paddleX && ball.x <= paddleX + PADDLE_W && ball.vy > 0) {
        const hitPos = (ball.x - paddleX) / PADDLE_W - 0.5; // -0.5..0.5
        ball.vx = hitPos * 7;
        ball.vy = -Math.abs(ball.vy);
      }

      // Brick collision
      outer: for (let r = 0; r < BRICK_ROWS; r++) {
        for (let c = 0; c < BRICK_COLS; c++) {
          if (!bricks[r][c]) continue;
          const bx = c * BRICK_W, by = r * BRICK_H + 30;
          if (ball.x + BALL_R > bx && ball.x - BALL_R < bx + BRICK_W &&
              ball.y + BALL_R > by && ball.y - BALL_R < by + BRICK_H) {
            bricks[r][c] = false;
            ball.vy *= -1;
            score += 10;
            dispatch('score', score);
            break outer;
          }
        }
      }

      if (bricks.every((row) => row.every((b) => !b))) {
        won = true;
        over = true;
        dispatch('gameOver', score);
      }

      if (ball.y > H + BALL_R) {
        lives--;
        if (lives <= 0) {
          over = true;
          dispatch('gameOver', score);
        } else {
          resetBall();
        }
      }
    }
    draw();
    raf = requestAnimationFrame(step);
  }

  function draw() {
    if (!ctx) return;
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, W, H);

    for (let r = 0; r < BRICK_ROWS; r++) {
      for (let c = 0; c < BRICK_COLS; c++) {
        if (!bricks[r][c]) continue;
        ctx.fillStyle = BRICK_COLORS[r % BRICK_COLORS.length];
        ctx.fillRect(c * BRICK_W + 1, r * BRICK_H + 30, BRICK_W - 2, BRICK_H - 2);
      }
    }

    ctx.fillStyle = '#60a5fa';
    ctx.fillRect(paddleX, H - 20, PADDLE_W, PADDLE_H);

    ctx.beginPath();
    ctx.arc(ball.x, ball.y, BALL_R, 0, Math.PI * 2);
    ctx.fillStyle = '#f8fafc';
    ctx.fill();

    ctx.fillStyle = '#94a3b8';
    ctx.font = '12px sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText(`Lives: ${lives}`, 6, 16);

    if (over) {
      ctx.fillStyle = 'rgba(15,23,42,0.8)';
      ctx.fillRect(0, 0, W, H);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 20px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(won ? 'You cleared it!' : 'Game Over', W / 2, H / 2 - 10);
      ctx.font = '13px sans-serif';
      ctx.fillText('Press Space to retry', W / 2, H / 2 + 14);
    } else if (paused) {
      ctx.fillStyle = 'rgba(15,23,42,0.6)';
      ctx.fillRect(0, 0, W, H);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 18px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Paused', W / 2, H / 2);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft' || e.key === 'a' || e.key === 'A') leftHeld = true;
    if (e.key === 'ArrowRight' || e.key === 'd' || e.key === 'D') rightHeld = true;
    if (e.key === ' ') { e.preventDefault(); if (over) reset(); else paused = !paused; }
  }
  function handleKeyUp(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft' || e.key === 'a' || e.key === 'A') leftHeld = false;
    if (e.key === 'ArrowRight' || e.key === 'd' || e.key === 'D') rightHeld = false;
  }
  function handlePointerMove(e: PointerEvent) {
    const rect = canvasEl.getBoundingClientRect();
    paddleX = Math.max(0, Math.min(W - PADDLE_W, e.clientX - rect.left - PADDLE_W / 2));
  }

  onMount(() => {
    ctx = canvasEl.getContext('2d')!;
    reset();
    raf = requestAnimationFrame(step);
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
  });
  onDestroy(() => {
    cancelAnimationFrame(raf);
    window.removeEventListener('keydown', handleKeyDown);
    window.removeEventListener('keyup', handleKeyUp);
  });
</script>

<div class="flex flex-col items-center gap-3">
  <div class="flex items-center gap-4 text-sm text-slate-300">
    <span>Score: <strong class="text-white">{score}</strong></span>
    <button on:click={() => (paused = !paused)} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">
      {paused ? 'Resume' : 'Pause'}
    </button>
    <button on:click={reset} class="px-2 py-1 bg-slate-800 hover:bg-slate-700 rounded-lg text-xs">Restart</button>
  </div>
  <canvas bind:this={canvasEl} width={W} height={H} class="rounded-xl border border-white/10 cursor-none"
    on:pointermove={handlePointerMove} />
  <p class="text-xs text-slate-500">Move the mouse, or ←/→ / A/D. Space to pause or retry.</p>
</div>
