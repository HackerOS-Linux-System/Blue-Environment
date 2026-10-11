import '../app.css';
import './phones.css';
import PhoneShell from './PhoneShell.svelte';

// Entry point of the PHONE shell. It is only ever bundled when the frontend is
// built with `VITE_BLUE_UI=phones` (`hl build.hl shell phones`, `npm run
// build:phones`): vite.config.ts points index.html at this file instead of
// ../main.ts in that case, so a regular build contains none of this code.
const app = new PhoneShell({ target: document.getElementById('root')! });

export default app;
