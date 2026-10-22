import '../app.css';
import './tv.css';
import TvShell from './TvShell.svelte';
import { mount } from 'svelte';

// Entry point of the TV ("10-foot UI") shell. Only bundled when the frontend
// is built with `VITE_BLUE_UI=tv` (`hl build.hl shell tv`, `npm run build:tv`):
// vite.config.ts points index.html here instead of ../main.ts in that case, so
// a regular build contains none of this code.
const app = mount(TvShell, { target: document.getElementById('root')! });

export default app;
