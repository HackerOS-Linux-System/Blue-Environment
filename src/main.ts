import './app.css';
import App from './App.svelte';

// NOTE: the original React entry point also routed to alternative shells
// (TV/"big picture" mode, Android mode, or a single-app standalone window)
// based on VITE_BLUE_UI_MODE / VITE_BLUE_ALTERNATIVE / VITE_BLUE_STANDALONE_APP
// env vars, set by `build.rb tv|android|app`. Those alternative shells still
// live under src/lib/alternatives/ as React and have not been ported yet —
// see STATUS.md. This entry point boots the main desktop shell only.

const app = new App({
  target: document.getElementById('root')!,
});

export default app;
