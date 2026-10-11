import './app.css';
import App from './App.svelte';

// Entry point of the DEFAULT (desktop) shell.
//
// The phone and TV shells have their own entry points — src/phones/main.ts and
// src/tv/main.ts. vite.config.ts swaps this file for one of them in index.html
// when the frontend is built with VITE_BLUE_UI=phones|tv (see `hl build.hl
// shell phones|tv` and the `build:phones` / `build:tv` npm scripts). Nothing
// here imports them, so a regular build never contains their code.

const app = new App({
  target: document.getElementById('root')!,
});

export default app;
