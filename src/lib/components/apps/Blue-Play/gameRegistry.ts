import SnakeGame from './games/SnakeGame.svelte';
import Game2048 from './games/Game2048.svelte';
import BreakoutGame from './games/BreakoutGame.svelte';
import TetrisGame from './games/TetrisGame.svelte';
import MemoryGame from './games/MemoryGame.svelte';
import PongGame from './games/PongGame.svelte';
import FlapGame from './games/FlapGame.svelte';
import MinesweeperGame from './games/MinesweeperGame.svelte';
import ReflexGame from './games/ReflexGame.svelte';
import ConnectFourGame from './games/ConnectFourGame.svelte';

export interface GameDef {
  id: string;
  title: string;
  tagline: string;
  color: string; // tailwind gradient classes for the library card
  component: any;
  controls: string;
}

// Blue Play's own original, built-in games. Each is a self-contained
// Svelte component (own game loop, own input handling) — the registry
// just wires it into the library grid and the shared save/high-score
// plumbing in BluePlay.svelte.
export const GAMES: GameDef[] = [
  {
    id: 'snake',
    title: 'Blue Snake',
    tagline: 'Classic grid snake — eat, grow, don\u2019t hit yourself.',
    color: 'from-emerald-500/30 to-emerald-900/40',
    component: SnakeGame,
    controls: 'Arrow keys / WASD to steer. Space to pause.',
  },
  {
    id: '2048',
    title: 'Blue 2048',
    tagline: 'Slide and merge numbered tiles up to 2048 (and beyond).',
    color: 'from-amber-500/30 to-amber-900/40',
    component: Game2048,
    controls: 'Arrow keys / WASD to slide tiles.',
  },
  {
    id: 'breakout',
    title: 'Blue Breakout',
    tagline: 'Bounce the ball, clear every brick, don\u2019t lose your lives.',
    color: 'from-sky-500/30 to-sky-900/40',
    component: BreakoutGame,
    controls: 'Mouse or ←/→ / A/D to move the paddle.',
  },
  {
    id: 'tetris',
    title: 'Blue Blocks',
    tagline: 'Falling tetromino puzzle \u2014 clear lines, chase combos.',
    color: 'from-violet-500/30 to-violet-900/40',
    component: TetrisGame,
    controls: '←/→/↓ move, ↑ rotate, Space hard drop, P pause.',
  },
  {
    id: 'memory',
    title: 'Blue Memory',
    tagline: 'Flip cards, find every pair, beat your move count.',
    color: 'from-rose-500/30 to-rose-900/40',
    component: MemoryGame,
    controls: 'Click two cards to find a matching pair.',
  },
  {
    id: 'pong',
    title: 'Blue Pong',
    tagline: 'Classic paddle duel against an AI opponent.',
    color: 'from-cyan-500/30 to-cyan-900/40',
    component: PongGame,
    controls: 'Mouse or ↑/↓ / W/S to move your paddle.',
  },
  {
    id: 'flap',
    title: 'Blue Flap',
    tagline: 'One-button flying \u2014 thread the gaps, beat your best.',
    color: 'from-yellow-500/30 to-yellow-900/40',
    component: FlapGame,
    controls: 'Click or press Space to flap.',
  },
  {
    id: 'minesweeper',
    title: 'Blue Sweeper',
    tagline: 'Classic minesweeper \u2014 clear the board without setting one off.',
    color: 'from-slate-500/30 to-slate-800/40',
    component: MinesweeperGame,
    controls: 'Left click to reveal, right click to flag.',
  },
  {
    id: 'reflex',
    title: 'Blue Reflex',
    tagline: 'Whack the target the instant it appears \u2014 30 second sprints.',
    color: 'from-orange-500/30 to-orange-900/40',
    component: ReflexGame,
    controls: 'Click the target as fast as you can.',
  },
  {
    id: 'connect4',
    title: 'Blue Connect',
    tagline: 'Local 2-player \u2014 get four in a row before they do.',
    color: 'from-indigo-500/30 to-indigo-900/40',
    component: ConnectFourGame,
    controls: 'Click a column to drop a piece. Pass the controls between turns.',
  },
];

export function getGame(id: string): GameDef | undefined {
  return GAMES.find((g) => g.id === id);
}
